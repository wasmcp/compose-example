# wasmcp Dynamic Tool Composition

This document explains the **dynamic tool composition pattern** used throughout this project's components.

## Table of Contents

- [Overview](#overview)
- [The Dynamic Composition Pattern](#the-dynamic-composition-pattern)
- [Component Types](#component-types)
- [Pipeline Ordering](#pipeline-ordering)
- [Multi-Level Composition](#multi-level-composition)
- [Implementation Patterns](#implementation-patterns)

## Overview

wasmcp components use a **handler chain pattern** for composition. Instead of statically importing tool interfaces, middleware components dynamically discover and call downstream tools through the `wasmcp:server/handler` interface.

**Key Benefits:**
- **Loose coupling** - Middleware doesn't import specific tool interfaces
- **Runtime flexibility** - Tools discovered and called dynamically at runtime
- **Deep composition** - Middleware can call OTHER middleware
- **Simple mental model** - Pipeline order explicitly defines dependencies

## The Dynamic Composition Pattern

**How it works:**

1. **Request arrives** at transport layer
2. **Flows through pipeline** - each component can:
   - Handle the request itself
   - Delegate to downstream via `downstream::handle_request()`
3. **Tool discovery** - `tools/list` merges results from entire chain
4. **Tool execution** - `tools/call` routes to the component providing that tool

### Example Flow

```
Client Request (pythagorean a=3, b=4)
    ↓
[pythagorean-middleware]
    ├─> Call square(3) → downstream
    │   ↓
    │   [math] → returns 9
    │   ↑
    ├─< Receives 9
    ├─> Call square(4) → downstream
    │   ↓
    │   [math] → returns 16
    │   ↑
    ├─< Receives 16
    ├─> Calculate sum: 9 + 16 = 25
    ├─> Call square_root(25) → downstream
    │   ↓
    │   [math] → returns 5
    │   ↑
    └─< Returns 5 to client
```

## Component Types

### Tool Components

Export `wasmcp:protocol/tools` to provide primitive operations.

**WIT Pattern:**
```wit
world tool-component {
    export wasmcp:protocol/tools;
}
```

**Purpose:** Provide atomic operations that can be composed by middleware.

**Examples in this project:**
- `math/` - Arithmetic and mathematical operations
- `statistics/` - Statistical primitives (mean, sum, count)
- `string-utils/` - String manipulation
- `system-info/` - System utilities

### Middleware Components

Import/export `wasmcp:server/handler` to orchestrate downstream tools.

**WIT Pattern:**
```wit
world middleware-component {
    import wasmcp:server/handler;  // Call downstream
    export wasmcp:server/handler;  // Accept upstream calls
}
```

**Purpose:** Compose primitive operations into higher-level capabilities.

**Examples in this project:**
- `pythagorean-middleware/` - Composes square + square_root
- `distance-calculator/` - Composes square + add + square_root
- `variance-middleware/` - Composes mean + arithmetic
- `stddev-middleware/` - Composes variance + square_root (middleware → middleware!)

## Pipeline Ordering

**CRITICAL CONCEPT:** Middleware must come BEFORE the tools it needs in the composition pipeline.

### Why Order Matters

The handler chain flows **left to right**:

```
Client → [Comp 1] → [Comp 2] → [Comp 3] → Method Not Found
```

When a component calls `downstream::handle_request()`, it reaches components to its **right**.

### Correct vs Incorrect

```bash
# ✅ CORRECT - middleware can reach tools
wasmcp compose pythagorean-middleware math -o server.wasm

# Pipeline: transport → pythagorean → math → method-not-found
# pythagorean calls square → reaches math downstream ✓
```

```bash
# ❌ WRONG - tools are upstream (unreachable)
wasmcp compose math pythagorean-middleware -o server.wasm

# Pipeline: transport → math → pythagorean → method-not-found
# pythagorean calls square → math is UPSTREAM ✗
```

### Error Messages as Guidance

When tools are missing, middleware provides helpful errors:

```
Tool 'square' not found in downstream handlers.
Ensure math comes AFTER pythagorean-middleware in the pipeline.
```

These runtime errors guide users to correct the pipeline ordering.

## Multi-Level Composition

One of the most powerful aspects of this pattern: **middleware can call OTHER middleware**.

### Example: Standard Deviation

`stddev-middleware` calls `variance-middleware`, which calls `statistics`:

```
stddev-middleware
    ↓ variance(numbers)
variance-middleware
    ↓ mean(numbers)
statistics
    ↑ returns mean
variance-middleware
    ↓ calculates variance
    ↑ returns variance
stddev-middleware
    ↓ square_root(variance)
math
    ↑ returns sqrt
stddev-middleware
    ↑ returns result
```

**Composition tree:**

```
┌─────────────────────┐
│ stddev-middleware   │  σ = √variance
└──────────┬──────────┘
           │
    ┌──────┴──────┐
    │             │
┌───▼──────────┐  │
│ variance-mw  │  │  Var = Σ(x-μ)²/n
└───┬──────────┘  │
    │             │
┌───▼─────────┐ ┌─▼─────────┐
│ statistics  │ │   math    │
└─────────────┘ └───────────┘
```

This proves the pattern supports **arbitrary composition depth**, not just single-level orchestration.

## Implementation Patterns

### Required Trait Implementation

All middleware must implement these exact signatures:

```rust
use bindings::exports::wasmcp::server::handler::Guest;
use bindings::wasmcp::server::handler as downstream;

impl Guest for MyMiddleware {
    fn handle_request(
        ctx: Context,
        request: (ClientRequest, RequestId),
        client_stream: Option<&OutputStream>,  // Note: reference!
    ) -> Result<ServerResponse, ErrorCode> {
        let (req, id) = request;
        match req {
            ClientRequest::ToolsList(list_req) => {
                handle_tools_list(list_req, id, &ctx, client_stream)
            }
            ClientRequest::ToolsCall(ref call_req) => {  // Note: ref to avoid move
                if call_req.name == "my_tool" {
                    handle_my_tool(call_req.clone(), id, &ctx, client_stream)
                } else {
                    downstream::handle_request(&ctx, (&req, &id), client_stream)
                }
            }
            _ => downstream::handle_request(&ctx, (&req, &id), client_stream),
        }
    }

    fn handle_notification(ctx: Context, notification: ClientNotification) {
        downstream::handle_notification(&ctx, &notification);
    }

    fn handle_response(
        ctx: Context,
        response: Result<(ClientResponse, RequestId), ErrorCode>
    ) {
        downstream::handle_response(&ctx, response);
    }
}
```

### Calling Downstream Tools

```rust
fn call_downstream_tool(
    ctx: &Context,
    tool_name: &str,
    arguments: Option<String>,
    request_id: &RequestId,
    client_stream: Option<&OutputStream>,
) -> Result<f64, String> {
    let tool_request = CallToolRequest {
        name: tool_name.to_string(),
        arguments,
    };

    let downstream_req = ClientRequest::ToolsCall(tool_request);

    match downstream::handle_request(ctx, (&downstream_req, request_id), client_stream) {
        Ok(ServerResponse::ToolsCall(result)) => {
            extract_number_from_result(&result)
        }
        Err(ErrorCode::MethodNotFound(_)) => {
            Err(format!(
                "Tool '{}' not found. Check pipeline ordering.",
                tool_name
            ))
        }
        Err(e) => Err(format!("Error calling '{}': {:?}", tool_name, e)),
        _ => Err("Unexpected response type".to_string()),
    }
}
```

### Merging Tools in tools/list

Middleware should merge their tools with downstream tools:

```rust
fn handle_tools_list(...) -> Result<ServerResponse, ErrorCode> {
    // Get downstream tools
    let downstream_req = ClientRequest::ToolsList(req.clone());
    let mut tools = match downstream::handle_request(ctx, (&downstream_req, &id), client_stream) {
        Ok(ServerResponse::ToolsList(result)) => result.tools,
        Err(ErrorCode::MethodNotFound(_)) => vec![],  // No downstream tools
        _ => vec![],
    };

    // Add our tool(s)
    tools.push(Tool {
        name: "my_tool".to_string(),
        input_schema: r#"{"type": "object", "properties": {...}}"#.to_string(),
        options: Some(ToolOptions {
            description: Some("Tool description".to_string()),
            title: Some("Tool Title".to_string()),
            meta: None,
            annotations: None,
            output_schema: None,
        }),
    });

    Ok(ServerResponse::ToolsList(ListToolsResult {
        tools,
        next_cursor: None,
        meta: None,
    }))
}
```

### Key Implementation Details

1. **Use `ref` in pattern matching** when you need to reuse the request:
   ```rust
   ClientRequest::ToolsCall(ref call_req) => { ... }
   ```

2. **Always forward notifications and responses** to maintain bidirectional flow

3. **Handle MethodNotFound gracefully** - it's normal for end of chain

4. **Provide helpful error messages** that guide correct pipeline ordering

5. **Extract results carefully** - handle both success and error cases:
   ```rust
   fn extract_number_from_result(result: &CallToolResult) -> Result<f64, String> {
       if result.is_error == Some(true) {
           return Err("Tool returned an error".to_string());
       }

       if let Some(ContentBlock::Text(text_content)) = result.content.first() {
           if let TextData::Text(text_str) = &text_content.text {
               return text_str.trim().parse::<f64>()
                   .map_err(|e| format!("Parse error: {}", e));
           }
       }

       Err("No text content in result".to_string())
   }
   ```

## Design Principles

This pattern embodies several key design principles:

1. **Composition over Inheritance** - Build complex behavior by combining simple components
2. **Runtime Flexibility** - Discovery and binding happen at runtime, not compile time
3. **Loose Coupling** - Components communicate through standard interfaces, not direct imports
4. **Incremental Complexity** - Start with primitives, layer on middleware as needed
