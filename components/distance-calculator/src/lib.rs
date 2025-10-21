//! Distance Calculator Middleware Component
//!
//! Calculates the Euclidean distance between two points in 2D space.
//! Formula: d = √((x2-x1)² + (y2-y1)²)
//!
//! This middleware demonstrates dynamic tool composition by orchestrating
//! multiple downstream math tool calls without static WIT imports.

#![allow(warnings)]

mod bindings {
    wit_bindgen::generate!({
        world: "distance-calculator",
        generate_all,
    });
}

use bindings::exports::wasmcp::server::handler::Guest;
use bindings::wasmcp::protocol::mcp::*;
use bindings::wasmcp::protocol::server_messages::Context;
use bindings::wasmcp::server::handler as downstream;
use bindings::wasi::io::streams::OutputStream;

struct DistanceCalculator;

impl Guest for DistanceCalculator {
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
                if call_req.name == "distance" {
                    handle_distance_call(call_req.clone(), id, &ctx, client_stream)
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

    // Add our distance tool
    tools.push(Tool {
        name: "distance".to_string(),
        input_schema: r#"{
            "type": "object",
            "properties": {
                "x1": {"type": "number", "description": "X coordinate of first point"},
                "y1": {"type": "number", "description": "Y coordinate of first point"},
                "x2": {"type": "number", "description": "X coordinate of second point"},
                "y2": {"type": "number", "description": "Y coordinate of second point"}
            },
            "required": ["x1", "y1", "x2", "y2"]
        }"#
        .to_string(),
        options: Some(ToolOptions {
            meta: None,
            annotations: None,
            description: Some(
                "Calculate Euclidean distance between two points: d = √((x2-x1)² + (y2-y1)²)"
                    .to_string(),
            ),
            output_schema: None,
            title: Some("2D Distance Calculator".to_string()),
        }),
    });

    Ok(ServerResponse::ToolsList(ListToolsResult {
        tools,
        next_cursor: None,
        meta: None,
    }))
}

fn handle_distance_call(
    request: CallToolRequest,
    id: RequestId,
    ctx: &Context,
    client_stream: Option<&OutputStream>,
) -> Result<ServerResponse, ErrorCode> {
    // Parse arguments
    let (x1, y1, x2, y2) = match parse_distance_args(&request.arguments) {
        Ok(coords) => coords,
        Err(msg) => return Ok(ServerResponse::ToolsCall(error_result(msg))),
    };

    // Step 1: Calculate dx = x2 - x1
    let dx = x2 - x1;

    // Step 2: Calculate dy = y2 - y1
    let dy = y2 - y1;

    // Step 3: Calculate dx²
    let dx_squared = match call_downstream_tool(
        ctx,
        &CallToolRequest {
            name: "square".to_string(),
            arguments: Some(format!(r#"{{"x": {}}}"#, dx)),
        },
        &id,
        client_stream,
    ) {
        Ok(val) => val,
        Err(e) => return Ok(ServerResponse::ToolsCall(error_result(e))),
    };

    // Step 4: Calculate dy²
    let dy_squared = match call_downstream_tool(
        ctx,
        &CallToolRequest {
            name: "square".to_string(),
            arguments: Some(format!(r#"{{"x": {}}}"#, dy)),
        },
        &id,
        client_stream,
    ) {
        Ok(val) => val,
        Err(e) => return Ok(ServerResponse::ToolsCall(error_result(e))),
    };

    // Step 5: Calculate sum = dx² + dy²
    let sum = match call_downstream_tool(
        ctx,
        &CallToolRequest {
            name: "add".to_string(),
            arguments: Some(format!(r#"{{"a": {}, "b": {}}}"#, dx_squared, dy_squared)),
        },
        &id,
        client_stream,
    ) {
        Ok(val) => val,
        Err(e) => return Ok(ServerResponse::ToolsCall(error_result(e))),
    };

    // Step 6: Calculate distance = √sum
    let distance = match call_downstream_tool(
        ctx,
        &CallToolRequest {
            name: "square_root".to_string(),
            arguments: Some(format!(r#"{{"x": {}}}"#, sum)),
        },
        &id,
        client_stream,
    ) {
        Ok(val) => val,
        Err(e) => return Ok(ServerResponse::ToolsCall(error_result(e))),
    };

    Ok(ServerResponse::ToolsCall(success_result(
        distance.to_string(),
    )))
}

fn call_downstream_tool(
    ctx: &Context,
    tool_request: &CallToolRequest,
    request_id: &RequestId,
    client_stream: Option<&OutputStream>,
) -> Result<f64, String> {
    let downstream_req = ClientRequest::ToolsCall(tool_request.clone());

    match downstream::handle_request(ctx, (&downstream_req, request_id), client_stream) {
        Ok(ServerResponse::ToolsCall(result)) => extract_number_from_result(&result),
        Err(ErrorCode::MethodNotFound(_)) => Err(format!(
            "Tool '{}' not found. Ensure required components \
             come AFTER this middleware in the pipeline.",
            tool_request.name
        )),
        Err(e) => Err(format!("Error calling '{}': {:?}", tool_request.name, e)),
        _ => Err("Unexpected response type".to_string()),
    }
}

fn parse_distance_args(arguments: &Option<String>) -> Result<(f64, f64, f64, f64), String> {
    let args_str = arguments
        .as_ref()
        .ok_or_else(|| "Missing arguments".to_string())?;

    let json: serde_json::Value =
        serde_json::from_str(args_str).map_err(|e| format!("Invalid JSON arguments: {}", e))?;

    let x1 = json
        .get("x1")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "Missing or invalid parameter 'x1'".to_string())?;

    let y1 = json
        .get("y1")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "Missing or invalid parameter 'y1'".to_string())?;

    let x2 = json
        .get("x2")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "Missing or invalid parameter 'x2'".to_string())?;

    let y2 = json
        .get("y2")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "Missing or invalid parameter 'y2'".to_string())?;

    Ok((x1, y1, x2, y2))
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

bindings::export!(DistanceCalculator with_types_in bindings);
