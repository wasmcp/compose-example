//! Statistics Tools Capability Provider
//!
//! A tools capability that provides statistical operations on arrays of numbers:
//! - mean: Calculate average
//! - sum: Calculate total
//! - count: Count elements

mod bindings {
    wit_bindgen::generate!({
        world: "statistics",
        generate_all,
    });
}

use bindings::exports::wasmcp::protocol::tools::Guest;
use bindings::wasmcp::protocol::mcp::*;
use bindings::wasi::io::streams::OutputStream;

struct Statistics;

impl Guest for Statistics {
    fn list_tools(
        _ctx: bindings::wasmcp::protocol::server_messages::Context,
        _request: ListToolsRequest,
        _client_stream: Option<&OutputStream>,
    ) -> Result<ListToolsResult, ErrorCode> {
        Ok(ListToolsResult {
            tools: vec![
                Tool {
                    name: "mean".to_string(),
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
                        description: Some("Calculate the mean (average) of an array of numbers".to_string()),
                        output_schema: None,
                        title: Some("Mean (Average)".to_string()),
                    }),
                },
                Tool {
                    name: "sum".to_string(),
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
                        description: Some("Calculate the sum of an array of numbers".to_string()),
                        output_schema: None,
                        title: Some("Sum".to_string()),
                    }),
                },
                Tool {
                    name: "count".to_string(),
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
                        description: Some("Count the number of elements in an array".to_string()),
                        output_schema: None,
                        title: Some("Count".to_string()),
                    }),
                },
            ],
            next_cursor: None,
            meta: None,
        })
    }

    fn call_tool(
        _ctx: bindings::wasmcp::protocol::server_messages::Context,
        request: CallToolRequest,
        _client_stream: Option<&OutputStream>,
    ) -> Option<CallToolResult> {
        match request.name.as_str() {
            "mean" => Some(execute_mean(&request.arguments)),
            "sum" => Some(execute_sum(&request.arguments)),
            "count" => Some(execute_count(&request.arguments)),
            _ => None, // We don't handle this tool
        }
    }
}

fn execute_mean(arguments: &Option<String>) -> CallToolResult {
    match parse_numbers(arguments) {
        Ok(numbers) => {
            if numbers.is_empty() {
                return error_result("Error: Cannot calculate mean of empty array".to_string());
            }
            let sum: f64 = numbers.iter().sum();
            let mean = sum / numbers.len() as f64;
            success_result(mean.to_string())
        }
        Err(msg) => error_result(msg),
    }
}

fn execute_sum(arguments: &Option<String>) -> CallToolResult {
    match parse_numbers(arguments) {
        Ok(numbers) => {
            let sum: f64 = numbers.iter().sum();
            success_result(sum.to_string())
        }
        Err(msg) => error_result(msg),
    }
}

fn execute_count(arguments: &Option<String>) -> CallToolResult {
    match parse_numbers(arguments) {
        Ok(numbers) => {
            success_result(numbers.len().to_string())
        }
        Err(msg) => error_result(msg),
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

bindings::export!(Statistics with_types_in bindings);
