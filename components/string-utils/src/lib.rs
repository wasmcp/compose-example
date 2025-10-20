//! string-utils Tools Capability Provider
//!
//! A tools capability that provides string manipulation operations.

mod bindings {
    wit_bindgen::generate!({
        world: "string-utils",
        generate_all,
    });
}

use bindings::exports::wasmcp::mcp::tools_capability::Guest;
use bindings::wasmcp::mcp::protocol::*;

struct StringUtils;

impl Guest for StringUtils {
    fn list_tools(_request: ListToolsRequest, _client: ClientContext) -> ListToolsResult {
        ListToolsResult {
            tools: vec![
                Tool {
                    name: "uppercase".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {
                            "text": {"type": "string", "description": "Text to convert to uppercase"}
                        },
                        "required": ["text"]
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Convert text to uppercase".to_string()),
                        output_schema: None,
                        title: Some("Uppercase".to_string()),
                    }),
                },
                Tool {
                    name: "lowercase".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {
                            "text": {"type": "string", "description": "Text to convert to lowercase"}
                        },
                        "required": ["text"]
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Convert text to lowercase".to_string()),
                        output_schema: None,
                        title: Some("Lowercase".to_string()),
                    }),
                },
                Tool {
                    name: "reverse".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {
                            "text": {"type": "string", "description": "Text to reverse"}
                        },
                        "required": ["text"]
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Reverse a string".to_string()),
                        output_schema: None,
                        title: Some("Reverse".to_string()),
                    }),
                },
                Tool {
                    name: "word_count".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {
                            "text": {"type": "string", "description": "Text to count words in"}
                        },
                        "required": ["text"]
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Count words in text".to_string()),
                        output_schema: None,
                        title: Some("Word Count".to_string()),
                    }),
                },
            ],
            next_cursor: None,
            meta: None,
        }
    }

    fn call_tool(request: CallToolRequest, _client: ClientContext) -> Option<CallToolResult> {
        match request.name.as_str() {
            "uppercase" => Some(execute_uppercase(&request.arguments)),
            "lowercase" => Some(execute_lowercase(&request.arguments)),
            "reverse" => Some(execute_reverse(&request.arguments)),
            "word_count" => Some(execute_word_count(&request.arguments)),
            _ => None, // We don't handle this tool
        }
    }
}

fn execute_uppercase(arguments: &Option<String>) -> CallToolResult {
    match parse_text_arg(arguments) {
        Ok(text) => success_result(text.to_uppercase()),
        Err(msg) => error_result(msg),
    }
}

fn execute_lowercase(arguments: &Option<String>) -> CallToolResult {
    match parse_text_arg(arguments) {
        Ok(text) => success_result(text.to_lowercase()),
        Err(msg) => error_result(msg),
    }
}

fn execute_reverse(arguments: &Option<String>) -> CallToolResult {
    match parse_text_arg(arguments) {
        Ok(text) => success_result(text.chars().rev().collect()),
        Err(msg) => error_result(msg),
    }
}

fn execute_word_count(arguments: &Option<String>) -> CallToolResult {
    match parse_text_arg(arguments) {
        Ok(text) => {
            let count = text.split_whitespace().count();
            success_result(format!("{} words", count))
        }
        Err(msg) => error_result(msg),
    }
}

fn parse_text_arg(arguments: &Option<String>) -> Result<String, String> {
    let args_str = arguments
        .as_ref()
        .ok_or_else(|| "Missing arguments".to_string())?;

    let json: serde_json::Value =
        serde_json::from_str(args_str).map_err(|e| format!("Invalid JSON arguments: {}", e))?;

    let text = json
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing or invalid parameter 'text'".to_string())?;

    Ok(text.to_string())
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

bindings::export!(StringUtils with_types_in bindings);