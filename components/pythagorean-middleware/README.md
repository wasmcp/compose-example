# Pythagorean Middleware - Dynamic Tool Composition Demo

A demonstration of **dynamic tool composition** in wasmcp middleware components.

## What This Demonstrates

This middleware showcases a powerful pattern: composing multiple tool calls dynamically through the server-handler chain without requiring static WIT imports.

### The Pattern

Instead of importing `wasmcp:protocol/tools`, this middleware:

1. **Discovers tools dynamically** via `tools/list` requests to downstream handlers
2. **Makes tool calls through the handler chain** using `ClientRequest::ToolsCall`
3. **Composes results** from multiple sequential tool calls
4. **Exposes a new composed tool** that orchestrates the underlying primitives

### The Pythagorean Tool

Calculates the hypotenuse of a right triangle using the Pythagorean theorem: **c = √(a² + b²)**

**Implementation:**
1. Call `square(a)` → get a²
2. Call `square(b)` → get b²
3. Add the results → a² + b²
4. Call `square_root(sum)` → get c

All calls go through `downstream::handle_request()` - no direct tool imports needed!

## Usage

### Pipeline Order Matters

The middleware must come **BEFORE** the tools it depends on in the pipeline:

```bash
# ✅ Correct - pythagorean-middleware before math
wasmcp compose \
  pythagorean-middleware \
  math \
  -o server.wasm

# ❌ Wrong - math before pythagorean-middleware
wasmcp compose \
  math \
  pythagorean-middleware \
  -o server.wasm
```

**Why?** Requests flow through the chain sequentially. When pythagorean-middleware calls `downstream::handle_request()`, it needs math to be downstream in the chain to handle those calls.

### Example Composition

```bash
# Build the components
wash build  # in math directory
wash build  # in pythagorean-middleware directory

# Compose them (order matters!)
wasmcp compose \
  pythagorean-middleware/build/pythagorean_middleware_s.wasm \
  math/build/math_s.wasm \
  -o pythag-server.wasm

# Run the server
wasmtime serve -Scli pythag-server.wasm
```

### API Example

```json
// List all available tools
POST http://localhost:8080/mcp
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list",
  "params": {}
}

// Response includes: square, square_root, power, pythagorean

// Calculate hypotenuse of 3-4-5 triangle
POST http://localhost:8080/mcp
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "pythagorean",
    "arguments": {"a": 3, "b": 4}
  }
}

// Response: {"content": [{"text": "5", "type": "text"}]}
```

## Architecture

### Component Structure

```wit
// pythagorean-middleware/wit/world.wit
world pythagorean-middleware {
    // Only needs handler interface - no tool imports!
    import wasmcp:server/handler@0.1.0;
    export wasmcp:server/handler@0.1.0;
}
```

### Runtime Composition

```
HTTP Request → Transport
    ↓ (handler chain)
pythagorean-middleware
    ↓ (if not "pythagorean" tool, delegate)
    ↓ (if "pythagorean", make downstream calls)
[wrapped math]
    ↓ (handles square, square_root, power, add, subtract, multiply, divide)
method-not-found
    ↓
HTTP Response
```

## Benefits of Dynamic Pattern

### ✅ Advantages

1. **No tight coupling** - Middleware doesn't import specific tool interfaces
2. **Runtime discovery** - Tools are discovered via `tools/list` at runtime
3. **Simple composition** - Just order components correctly in the pipeline
4. **Flexible** - Easy to swap tool implementations without rebuilding middleware
5. **Works with current wasmcp** - No framework changes needed!

### ⚠️ Trade-offs

1. **Order dependency** - Pipeline order must be correct
2. **Runtime errors** - Missing tools fail at runtime, not compile-time
3. **Error messages** - Need clear messages about ordering requirements

## Implementation Notes

### Key Functions

**`handle_tools_list()`**
- Calls downstream to get available tools
- Merges pythagorean tool definition with downstream tools
- Returns combined tool list

**`handle_pythagorean_call()`**
- Parses input arguments (a, b)
- Makes sequential downstream calls: square(a), square(b), square_root(sum)
- Composes results into final hypotenuse

**`call_downstream_tool()`**
- Wraps tool call in `ClientRequest::ToolsCall`
- Delegates to `downstream::handle_request()`
- Extracts numeric result from response

### Error Handling

Clear error messages guide users to fix ordering:

```
Tool 'square' not found in downstream handlers.
Ensure math comes AFTER pythagorean-middleware in the pipeline.
```

## Extending This Pattern

You can create your own composed tools using this pattern:

1. **Implement middleware** that exports `server-handler`
2. **Call downstream** for primitive operations
3. **Compose results** into new functionality
4. **Order correctly** in the pipeline

### Example: Distance Calculator

```rust
// Calculate distance between two points: √((x2-x1)² + (y2-y1)²)

fn handle_distance_call(...) {
    let dx = x2 - x1;
    let dy = y2 - y1;

    // Call square(dx)
    let dx_squared = call_downstream_tool("square", dx)?;

    // Call square(dy)
    let dy_squared = call_downstream_tool("square", dy)?;

    // Call square_root(dx² + dy²)
    let distance = call_downstream_tool("square_root", dx_squared + dy_squared)?;

    Ok(distance)
}
```

## See Also

- [math](../math/README.md) - Comprehensive math operations (arithmetic + primitives)
- [wasmcp documentation](https://github.com/wasmcp/wasmcp) - Component composition framework
