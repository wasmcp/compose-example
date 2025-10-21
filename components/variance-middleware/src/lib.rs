//! Variance Middleware Component
//!
//! Calculates sample variance of an array of numbers.
//! Formula: variance = Σ(x - μ)² / n where μ is the mean
//!
//! This demonstrates multi-level composition:
//! - Calls mean() to calculate average
//! - Calls subtract() and square() for each element
//! - Calculates final average of squared differences

#![allow(warnings)]

mod bindings {
    wit_bindgen::generate!({
        world: "variance-middleware",
        generate_all,
    });
}

use bindings::exports::wasmcp::server::handler::Guest;
use bindings::wasmcp::protocol::mcp::*;
use bindings::wasmcp::protocol::server_messages::Context;
use bindings::wasmcp::server::handler as downstream;
use bindings::wasi::io::streams::OutputStream;

struct VarianceMiddleware;

impl Guest for VarianceMiddleware {
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
                if call_req.name == "variance" {
                    handle_variance_call(call_req.clone(), id, &ctx, client_stream)
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

    // Add our variance tool
    tools.push(Tool {
        name: "variance".to_string(),
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
                "Calculate the variance of an array of numbers: Σ(x - μ)² / n".to_string(),
            ),
            output_schema: None,
            title: Some("Variance".to_string()),
        }),
    });

    Ok(ServerResponse::ToolsList(ListToolsResult {
        tools,
        next_cursor: None,
        meta: None,
    }))
}

fn handle_variance_call(
    request: CallToolRequest,
    id: RequestId,
    ctx: &Context,
    client_stream: Option<&OutputStream>,
) -> Result<ServerResponse, ErrorCode> {
    // Parse the numbers array
    let numbers = match parse_numbers(&request.arguments) {
        Ok(nums) => nums,
        Err(msg) => return Ok(ServerResponse::ToolsCall(error_result(msg))),
    };

    if numbers.is_empty() {
        return Ok(ServerResponse::ToolsCall(error_result(
            "Error: Cannot calculate variance of empty array".to_string(),
        )));
    }

    // Step 1: Calculate the mean
    let mean = match call_mean_tool(ctx, &numbers, &id, client_stream) {
        Ok(m) => m,
        Err(e) => return Ok(ServerResponse::ToolsCall(error_result(e))),
    };

    // Step 2: Calculate squared differences for each number
    let mut squared_diffs = Vec::new();
    for &num in &numbers {
        // Calculate (num - mean)²
        let diff = num - mean;
        let squared = diff * diff;
        squared_diffs.push(squared);
    }

    // Step 3: Calculate mean of squared differences
    let variance = squared_diffs.iter().sum::<f64>() / squared_diffs.len() as f64;

    Ok(ServerResponse::ToolsCall(success_result(
        variance.to_string(),
    )))
}

fn call_mean_tool(
    ctx: &Context,
    numbers: &[f64],
    request_id: &RequestId,
    client_stream: Option<&OutputStream>,
) -> Result<f64, String> {
    // Create JSON array for the mean tool
    let numbers_json = serde_json::to_string(numbers).map_err(|e| format!("JSON error: {}", e))?;

    let tool_request = CallToolRequest {
        name: "mean".to_string(),
        arguments: Some(format!(r#"{{"numbers": {}}}"#, numbers_json)),
    };

    let downstream_req = ClientRequest::ToolsCall(tool_request);

    match downstream::handle_request(ctx, (&downstream_req, request_id), client_stream) {
        Ok(ServerResponse::ToolsCall(result)) => extract_number_from_result(&result),
        Err(ErrorCode::MethodNotFound(_)) => Err(
            "Tool 'mean' not found. Ensure statistics component comes AFTER this middleware in the pipeline."
                .to_string(),
        ),
        Err(e) => Err(format!("Error calling 'mean': {:?}", e)),
        _ => Err("Unexpected response type".to_string()),
    }
}

fn parse_numbers(arguments: &Option<String>) -> Result<Vec<f64>, String> {
    let args_str = arguments
        .as_ref()
        .ok_or_else(|| "Missing arguments".to_string())?;

    let json: serde_json::Value =
        serde_json::from_str(args_str).map_err(|e| format!("Invalid JSON arguments: {}", e))?;

    let numbers_array = json
        .get("numbers")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Missing or invalid parameter 'numbers'".to_string())?;

    let numbers: Result<Vec<f64>, String> = numbers_array
        .iter()
        .map(|v| {
            v.as_f64()
                .ok_or_else(|| format!("Invalid number in array: {}", v))
        })
        .collect();

    numbers
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

bindings::export!(VarianceMiddleware with_types_in bindings);
