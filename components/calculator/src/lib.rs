//! calculator Tools Capability Provider
//!
//! A tools capability that provides basic arithmetic operations.

mod bindings {
    wit_bindgen::generate!({
        world: "calculator",
        generate_all,
    });
}

use bindings::exports::wasmcp::protocol::tools::Guest;
use bindings::wasmcp::protocol::mcp::*;
use bindings::wasi::io::streams::OutputStream;

struct Calculator;

impl Guest for Calculator {
    fn list_tools(
        _ctx: bindings::wasmcp::protocol::server_messages::Context,
        _request: ListToolsRequest,
        _client_stream: Option<&OutputStream>,
    ) -> Result<ListToolsResult, ErrorCode> {
        Ok(ListToolsResult {
            tools: vec![
                Tool {
                    name: "add".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {
                            "a": {"type": "number", "description": "First number"},
                            "b": {"type": "number", "description": "Second number"}
                        },
                        "required": ["a", "b"]
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Add two numbers together".to_string()),
                        output_schema: None,
                        title: Some("Add".to_string()),
                    }),
                },
                Tool {
                    name: "subtract".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {
                            "a": {"type": "number", "description": "Number to subtract from"},
                            "b": {"type": "number", "description": "Number to subtract"}
                        },
                        "required": ["a", "b"]
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Subtract b from a".to_string()),
                        output_schema: None,
                        title: Some("Subtract".to_string()),
                    }),
                },
                Tool {
                    name: "multiply".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {
                            "a": {"type": "number", "description": "First number"},
                            "b": {"type": "number", "description": "Second number"}
                        },
                        "required": ["a", "b"]
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Multiply two numbers".to_string()),
                        output_schema: None,
                        title: Some("Multiply".to_string()),
                    }),
                },
                Tool {
                    name: "divide".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {
                            "a": {"type": "number", "description": "Dividend"},
                            "b": {"type": "number", "description": "Divisor"}
                        },
                        "required": ["a", "b"]
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Divide a by b".to_string()),
                        output_schema: None,
                        title: Some("Divide".to_string()),
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
            "add" => Some(execute_operation(&request.arguments, |a, b| a + b)),
            "subtract" => Some(execute_operation(&request.arguments, |a, b| a - b)),
            "multiply" => Some(execute_operation(&request.arguments, |a, b| a * b)),
            "divide" => Some(execute_divide(&request.arguments)),
            _ => None, // We don't handle this tool
        }
    }
}

fn execute_operation<F>(arguments: &Option<String>, op: F) -> CallToolResult
where
    F: FnOnce(f64, f64) -> f64,
{
    match parse_args(arguments) {
        Ok((a, b)) => {
            let result = op(a, b);
            success_result(result.to_string())
        }
        Err(msg) => error_result(msg),
    }
}

fn execute_divide(arguments: &Option<String>) -> CallToolResult {
    match parse_args(arguments) {
        Ok((a, b)) => {
            if b == 0.0 {
                error_result("Error: Division by zero".to_string())
            } else {
                let result = a / b;
                success_result(result.to_string())
            }
        }
        Err(msg) => error_result(msg),
    }
}

fn parse_args(arguments: &Option<String>) -> Result<(f64, f64), String> {
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

bindings::export!(Calculator with_types_in bindings);
