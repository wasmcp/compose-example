//! Pythagorean Theorem Middleware Component
//!
//! A middleware that demonstrates dynamic tool composition by:
//! - Making tool calls through the server-handler chain (not via imports)
//! - Providing a "pythagorean" tool that orchestrates multiple downstream tool calls
//! - Delegating unknown requests downstream in the middleware chain
//!
//! ## Dynamic Tool Calling Pattern
//!
//! Instead of importing `wasmcp:protocol/tools`, this middleware:
//! 1. Calls `downstream::handle_request()` with `ToolsCall` requests
//! 2. Accumulates results from multiple sequential tool calls
//! 3. Composes them into a final result
//!
//! ## Order Dependency
//!
//! This middleware MUST come BEFORE components that provide the tools it needs:
//! - Requires: `square` tool (from math)
//! - Requires: `square_root` tool (from math)
//!
//! Correct pipeline order:
//! ```bash
//! wasmcp compose pythagorean-middleware math -o server.wasm
//! ```

#![allow(warnings)]

mod bindings {
    wit_bindgen::generate!({
        world: "pythagorean-middleware",
        generate_all,
    });
}

use bindings::exports::wasmcp::server::handler::Guest;
use bindings::wasi::io::streams::OutputStream;
use bindings::wasmcp::protocol::mcp::*;
use bindings::wasmcp::protocol::server_messages::Context;
use bindings::wasmcp::server::handler as downstream; // Downstream handler chain

struct PythagoreanMiddleware;

impl Guest for PythagoreanMiddleware {
    fn handle_request(
        ctx: Context,
        request: (ClientRequest, RequestId),
        client_stream: Option<&OutputStream>,
    ) -> Result<ServerResponse, ErrorCode> {
        let (req, id) = request;
        match req {
            ClientRequest::ToolsList(list_req) => {
                handle_tools_list(list_req, id, &ctx, client_stream)
            }
            ClientRequest::ToolsCall(ref call_req) => {
                if call_req.name == "pythagorean" {
                    handle_pythagorean_call(call_req.clone(), id, &ctx, client_stream)
                } else {
                    // Not our tool - delegate downstream
                    downstream::handle_request(&ctx, (&req, &id), client_stream)
                }
            }
            _ => {
                // Not a tool request - delegate downstream
                downstream::handle_request(&ctx, (&req, &id), client_stream)
            }
        }
    }

    fn handle_notification(ctx: Context, notification: ClientNotification) {
        // Forward to downstream handler
        downstream::handle_notification(&ctx, &notification);
    }

    fn handle_response(ctx: Context, response: Result<(ClientResponse, RequestId), ErrorCode>) {
        // Forward to downstream handler
        downstream::handle_response(&ctx, response);
    }
}

/// Handle tools/list - merge our pythagorean tool with downstream tools
fn handle_tools_list(
    req: ListToolsRequest,
    id: RequestId,
    ctx: &Context,
    client_stream: Option<&OutputStream>,
) -> Result<ServerResponse, ErrorCode> {
    // Get our pythagorean tool definition
    let pythagorean_tool = Tool {
        name: "pythagorean".to_string(),
        input_schema: r#"{
            "type": "object",
            "properties": {
                "a": {"type": "number", "description": "First side of right triangle"},
                "b": {"type": "number", "description": "Second side of right triangle"}
            },
            "required": ["a", "b"]
        }"#
        .to_string(),
        options: Some(ToolOptions {
            meta: None,
            annotations: None,
            description: Some(
                "Calculate the hypotenuse of a right triangle using the Pythagorean theorem (c = √(a² + b²))".to_string(),
            ),
            output_schema: None,
            title: Some("Pythagorean Theorem".to_string()),
        }),
    };

    // Get downstream tools by calling downstream handler with tools/list
    let downstream_req = ClientRequest::ToolsList(req.clone());
    match downstream::handle_request(ctx, (&downstream_req, &id), client_stream) {
        Ok(ServerResponse::ToolsList(mut downstream_result)) => {
            // Merge our tool with downstream tools
            downstream_result.tools.push(pythagorean_tool);
            Ok(ServerResponse::ToolsList(downstream_result))
        }
        Err(ErrorCode::MethodNotFound(_)) => {
            // Downstream doesn't support tools - just return ours
            Ok(ServerResponse::ToolsList(ListToolsResult {
                tools: vec![pythagorean_tool],
                next_cursor: None,
                meta: None,
            }))
        }
        Err(_) | Ok(_) => {
            // Unexpected response - return our tool
            Ok(ServerResponse::ToolsList(ListToolsResult {
                tools: vec![pythagorean_tool],
                next_cursor: None,
                meta: None,
            }))
        }
    }
}

/// Handle pythagorean tool call - make sequential downstream tool calls
fn handle_pythagorean_call(
    request: CallToolRequest,
    id: RequestId,
    ctx: &Context,
    client_stream: Option<&OutputStream>,
) -> Result<ServerResponse, ErrorCode> {
    // Parse arguments
    let (a, b) = match parse_pythagorean_args(&request.arguments) {
        Ok(values) => values,
        Err(msg) => {
            return Ok(ServerResponse::ToolsCall(error_result(msg)));
        }
    };

    // Step 1: Call square(a) through downstream handler chain
    let square_a_req = CallToolRequest {
        name: "square".to_string(),
        arguments: Some(format!(r#"{{"x": {}}}"#, a)),
    };

    let a_squared = match call_downstream_tool(ctx, &square_a_req, &id, client_stream) {
        Ok(result) => result,
        Err(msg) => return Ok(ServerResponse::ToolsCall(error_result(msg))),
    };

    // Step 2: Call square(b) through downstream handler chain
    let square_b_req = CallToolRequest {
        name: "square".to_string(),
        arguments: Some(format!(r#"{{"x": {}}}"#, b)),
    };

    let b_squared = match call_downstream_tool(ctx, &square_b_req, &id, client_stream) {
        Ok(result) => result,
        Err(msg) => return Ok(ServerResponse::ToolsCall(error_result(msg))),
    };

    // Step 3: Add the squared values
    let sum = a_squared + b_squared;

    // Step 4: Call square_root(sum) through downstream handler chain
    let sqrt_req = CallToolRequest {
        name: "square_root".to_string(),
        arguments: Some(format!(r#"{{"x": {}}}"#, sum)),
    };

    match call_downstream_tool(ctx, &sqrt_req, &id, client_stream) {
        Ok(hypotenuse) => {
            // Return the hypotenuse as the result
            Ok(ServerResponse::ToolsCall(success_result(
                hypotenuse.to_string(),
            )))
        }
        Err(msg) => Ok(ServerResponse::ToolsCall(error_result(msg))),
    }
}

/// Call a tool through the downstream handler chain and extract numeric result
fn call_downstream_tool(
    ctx: &Context,
    tool_request: &CallToolRequest,
    request_id: &RequestId,
    client_stream: Option<&OutputStream>,
) -> Result<f64, String> {
    // Make the downstream call
    let downstream_req = ClientRequest::ToolsCall(tool_request.clone());

    match downstream::handle_request(ctx, (&downstream_req, request_id), client_stream) {
        Ok(ServerResponse::ToolsCall(result)) => {
            // Extract the numeric value from the result
            extract_number_from_result(&result)
        }
        Ok(_) => Err(format!(
            "Unexpected response type when calling '{}'",
            tool_request.name
        )),
        Err(ErrorCode::MethodNotFound(_)) => Err(format!(
            "Tool '{}' not found in downstream handlers. \
             Ensure math comes AFTER pythagorean-middleware in the pipeline.",
            tool_request.name
        )),
        Err(e) => Err(format!("Error calling '{}': {:?}", tool_request.name, e)),
    }
}

/// Parse pythagorean arguments (a, b)
fn parse_pythagorean_args(arguments: &Option<String>) -> Result<(f64, f64), String> {
    let args_str = arguments
        .as_ref()
        .ok_or_else(|| "Missing arguments".to_string())?;

    let json: serde_json::Value =
        serde_json::from_str(args_str).map_err(|e| format!("Invalid JSON arguments: {}", e))?;

    let a = json
        .get("a")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "Missing or invalid parameter 'a'".to_string())?;

    let b = json
        .get("b")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "Missing or invalid parameter 'b'".to_string())?;

    Ok((a, b))
}

/// Extract a numeric value from a CallToolResult
fn extract_number_from_result(result: &CallToolResult) -> Result<f64, String> {
    // Check if it's an error result
    if result.is_error == Some(true) {
        return Err("Tool returned an error".to_string());
    }

    // Extract the text from the first content block
    if let Some(ContentBlock::Text(text_content)) = result.content.first() {
        if let TextData::Text(text_str) = &text_content.text {
            // Parse the text as a number
            text_str
                .trim()
                .parse::<f64>()
                .map_err(|e| format!("Failed to parse number from result: {}", e))
        } else {
            Err("Text content is a stream, not inline text".to_string())
        }
    } else {
        Err("No text content in result".to_string())
    }
}

fn success_result(result: String) -> CallToolResult {
    CallToolResult {
        content: vec![ContentBlock::Text(TextContent {
            text: TextData::Text(result),
            options: None,
        })],
        is_error: None,
        meta: None,
        structured_content: None,
    }
}

fn error_result(message: String) -> CallToolResult {
    CallToolResult {
        content: vec![ContentBlock::Text(TextContent {
            text: TextData::Text(message),
            options: None,
        })],
        is_error: Some(true),
        meta: None,
        structured_content: None,
    }
}

bindings::export!(PythagoreanMiddleware with_types_in bindings);
