//! Standard Deviation Middleware Component
//!
//! Calculates standard deviation by composing variance and square_root tools.
//! Formula: stddev = √(variance)
//!
//! This demonstrates MULTI-LEVEL COMPOSITION:
//! stddev-middleware → variance-middleware → statistics → math
//!
//! Shows middleware calling OTHER middleware, creating composition trees!

#![allow(warnings)]

mod bindings {
    wit_bindgen::generate!({
        world: "stddev-middleware",
        generate_all,
    });
}

use bindings::exports::wasmcp::server::handler::Guest;
use bindings::wasmcp::protocol::mcp::*;
use bindings::wasmcp::protocol::server_messages::Context;
use bindings::wasmcp::server::handler as downstream;
use bindings::wasi::io::streams::OutputStream;

struct StdDevMiddleware;

impl Guest for StdDevMiddleware {
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
                if call_req.name == "standard_deviation" || call_req.name == "stddev" {
                    handle_stddev_call(call_req.clone(), id, &ctx, client_stream)
                } else {
                    // Delegate to downstream handler
                    downstream::handle_request(&ctx, (&req, &id), client_stream)
                }
            }
            // Delegate all other requests to downstream
            _ => downstream::handle_request(&ctx, (&req, &id), client_stream),
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

fn handle_tools_list(
    req: ListToolsRequest,
    id: RequestId,
    ctx: &Context,
    client_stream: Option<&OutputStream>,
) -> Result<ServerResponse, ErrorCode> {
    // Get tools from downstream handlers
    let downstream_req = ClientRequest::ToolsList(req);
    let downstream_response =
        downstream::handle_request(ctx, (&downstream_req, &id), client_stream)?;

    // Extract the tools list from downstream response
    let mut tools = if let ServerResponse::ToolsList(result) = downstream_response {
        result.tools
    } else {
        vec![]
    };

    // Add our standard deviation tool
    tools.push(Tool {
        name: "standard_deviation".to_string(),
        input_schema: r#"{
            "type": "object",
            "properties": {
                "numbers": {
                    "type": "array",
                    "items": {"type": "number"},
                    "description": "Array of numbers"
                }
            },
            "required": ["numbers"]
        }"#
        .to_string(),
        options: Some(ToolOptions {
            meta: None,
            annotations: None,
            description: Some(
                "Calculate the standard deviation (σ) of an array of numbers: √(variance)"
                    .to_string(),
            ),
            output_schema: None,
            title: Some("Standard Deviation".to_string()),
        }),
    });

    // Also add a shorthand alias
    tools.push(Tool {
        name: "stddev".to_string(),
        input_schema: r#"{
            "type": "object",
            "properties": {
                "numbers": {
                    "type": "array",
                    "items": {"type": "number"},
                    "description": "Array of numbers"
                }
            },
            "required": ["numbers"]
        }"#
        .to_string(),
        options: Some(ToolOptions {
            meta: None,
            annotations: None,
            description: Some("Alias for standard_deviation".to_string()),
            output_schema: None,
            title: Some("StdDev (alias)".to_string()),
        }),
    });

    Ok(ServerResponse::ToolsList(ListToolsResult {
        tools,
        next_cursor: None,
        meta: None,
    }))
}

fn handle_stddev_call(
    request: CallToolRequest,
    id: RequestId,
    ctx: &Context,
    client_stream: Option<&OutputStream>,
) -> Result<ServerResponse, ErrorCode> {
    // Step 1: Call variance tool
    let variance = match call_variance_tool(ctx, &request.arguments, &id, client_stream) {
        Ok(v) => v,
        Err(e) => return Ok(ServerResponse::ToolsCall(error_result(e))),
    };

    // Step 2: Call square_root tool on the variance
    let stddev = match call_square_root_tool(ctx, variance, &id, client_stream) {
        Ok(sd) => sd,
        Err(e) => return Ok(ServerResponse::ToolsCall(error_result(e))),
    };

    Ok(ServerResponse::ToolsCall(success_result(
        stddev.to_string(),
    )))
}

fn call_variance_tool(
    ctx: &Context,
    arguments: &Option<String>,
    request_id: &RequestId,
    client_stream: Option<&OutputStream>,
) -> Result<f64, String> {
    let tool_request = CallToolRequest {
        name: "variance".to_string(),
        arguments: arguments.clone(),
    };

    let downstream_req = ClientRequest::ToolsCall(tool_request);

    match downstream::handle_request(ctx, (&downstream_req, request_id), client_stream) {
        Ok(ServerResponse::ToolsCall(result)) => extract_number_from_result(&result),
        Err(ErrorCode::MethodNotFound(_)) => Err(
            "Tool 'variance' not found. Ensure variance-middleware comes AFTER this middleware in the pipeline."
                .to_string(),
        ),
        Err(e) => Err(format!("Error calling 'variance': {:?}", e)),
        _ => Err("Unexpected response type".to_string()),
    }
}

fn call_square_root_tool(
    ctx: &Context,
    value: f64,
    request_id: &RequestId,
    client_stream: Option<&OutputStream>,
) -> Result<f64, String> {
    let tool_request = CallToolRequest {
        name: "square_root".to_string(),
        arguments: Some(format!(r#"{{"x": {}}}"#, value)),
    };

    let downstream_req = ClientRequest::ToolsCall(tool_request);

    match downstream::handle_request(ctx, (&downstream_req, request_id), client_stream) {
        Ok(ServerResponse::ToolsCall(result)) => extract_number_from_result(&result),
        Err(ErrorCode::MethodNotFound(_)) => Err(
            "Tool 'square_root' not found. Ensure math component comes AFTER this middleware in the pipeline."
                .to_string(),
        ),
        Err(e) => Err(format!("Error calling 'square_root': {:?}", e)),
        _ => Err("Unexpected response type".to_string()),
    }
}

fn extract_number_from_result(result: &CallToolResult) -> Result<f64, String> {
    if result.is_error == Some(true) {
        return Err("Tool call returned error".to_string());
    }

    for content in &result.content {
        if let ContentBlock::Text(text_content) = content {
            if let TextData::Text(text) = &text_content.text {
                return text
                    .parse::<f64>()
                    .map_err(|_| format!("Failed to parse result as number: {}", text));
            }
        }
    }

    Err("No text content found in result".to_string())
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

bindings::export!(StdDevMiddleware with_types_in bindings);
