//! system-info Tools Capability Provider
//!
//! A tools capability that provides system utility operations.

mod bindings {
    wit_bindgen::generate!({
        world: "system-info",
        generate_all,
    });
}

use bindings::exports::wasmcp::mcp::tools_capability::Guest;
use bindings::wasmcp::mcp::protocol::*;
use std::time::{SystemTime, UNIX_EPOCH};

struct SystemInfo;

impl Guest for SystemInfo {
    fn list_tools(_request: ListToolsRequest, _client: ClientContext) -> ListToolsResult {
        ListToolsResult {
            tools: vec![
                Tool {
                    name: "timestamp".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {},
                        "required": []
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Get current Unix timestamp".to_string()),
                        output_schema: None,
                        title: Some("Timestamp".to_string()),
                    }),
                },
                Tool {
                    name: "random_uuid".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {},
                        "required": []
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Generate a random UUID v4".to_string()),
                        output_schema: None,
                        title: Some("Random UUID".to_string()),
                    }),
                },
                Tool {
                    name: "base64_encode".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {
                            "text": {"type": "string", "description": "Text to encode to base64"}
                        },
                        "required": ["text"]
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Encode string to base64".to_string()),
                        output_schema: None,
                        title: Some("Base64 Encode".to_string()),
                    }),
                },
                Tool {
                    name: "base64_decode".to_string(),
                    input_schema: r#"{
                        "type": "object",
                        "properties": {
                            "text": {"type": "string", "description": "Base64 text to decode"}
                        },
                        "required": ["text"]
                    }"#
                    .to_string(),
                    options: Some(ToolOptions {
                        meta: None,
                        annotations: None,
                        description: Some("Decode base64 to string".to_string()),
                        output_schema: None,
                        title: Some("Base64 Decode".to_string()),
                    }),
                },
            ],
            next_cursor: None,
            meta: None,
        }
    }

    fn call_tool(request: CallToolRequest, _client: ClientContext) -> Option<CallToolResult> {
        match request.name.as_str() {
            "timestamp" => Some(execute_timestamp()),
            "random_uuid" => Some(execute_random_uuid()),
            "base64_encode" => Some(execute_base64_encode(&request.arguments)),
            "base64_decode" => Some(execute_base64_decode(&request.arguments)),
            _ => None, // We don't handle this tool
        }
    }
}

fn execute_timestamp() -> CallToolResult {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let timestamp = duration.as_secs();
            success_result(timestamp.to_string())
        }
        Err(e) => error_result(format!("Failed to get timestamp: {}", e)),
    }
}

fn execute_random_uuid() -> CallToolResult {
    // Simple UUID v4 generation
    // In production, you might want to use the uuid crate
    let uuid = format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        random_u32(),
        random_u16(),
        (random_u16() & 0x0fff) | 0x4000, // Version 4
        (random_u16() & 0x3fff) | 0x8000, // Variant 10
        random_u64() & 0xffffffffffff
    );
    success_result(uuid)
}

fn execute_base64_encode(arguments: &Option<String>) -> CallToolResult {
    match parse_text_arg(arguments) {
        Ok(text) => {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let encoded = STANDARD.encode(text.as_bytes());
            success_result(encoded)
        }
        Err(msg) => error_result(msg),
    }
}

fn execute_base64_decode(arguments: &Option<String>) -> CallToolResult {
    match parse_text_arg(arguments) {
        Ok(text) => {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            match STANDARD.decode(&text) {
                Ok(decoded_bytes) => {
                    match String::from_utf8(decoded_bytes) {
                        Ok(decoded_string) => success_result(decoded_string),
                        Err(_) => error_result("Decoded data is not valid UTF-8 text".to_string()),
                    }
                }
                Err(e) => error_result(format!("Invalid base64: {}", e)),
            }
        }
        Err(msg) => error_result(msg),
    }
}

// Simple random number generators for UUID
// In a real application, use a proper random number generator
fn random_u16() -> u16 {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u16;
    time.wrapping_mul(40503) // Simple hash (prime number that fits in u16)
}

fn random_u32() -> u32 {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u32;
    time.wrapping_mul(2654435761) // Simple hash
}

fn random_u64() -> u64 {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    time.wrapping_mul(11400714819323198485) // Simple hash
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

bindings::export!(SystemInfo with_types_in bindings);