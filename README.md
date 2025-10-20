# MCP Multi-Component Server

Example of building, composing, and deploying multiple MCP (Model Context Protocol) tool components as WebAssembly modules to multiple runtime environments using wasmcp.

## Overview

This project demonstrates:
- Building multiple MCP tool components using `wasmcp`
- Composing components into a single WASM module
- Deploying to multiple runtimes:
  - **wash** - Local wasmCloud development
  - **Cosmonic** - Kubernetes with Cosmonic Control
  - **wasmtime** - Direct WebAssembly runtime
- Using GitHub Container Registry (ghcr.io) for artifact storage

## Components

### 1. Calculator
Mathematical operations component providing:
- `add` - Add two numbers
- `subtract` - Subtract b from a
- `multiply` - Multiply two numbers
- `divide` - Divide a by b (with zero check)

### 2. String Utils
String manipulation component providing:
- `uppercase` - Convert text to uppercase
- `lowercase` - Convert text to lowercase
- `reverse` - Reverse a string
- `word_count` - Count words in text

### 3. System Info
System utility component providing:
- `timestamp` - Get current Unix timestamp
- `random_uuid` - Generate a random UUID v4
- `base64_encode` - Encode string to base64
- `base64_decode` - Decode base64 to string

## Quick Start

### Prerequisites

```bash
# Validate all dependencies
make validate
```

Required tools:
- Docker
- kind (Kubernetes in Docker) - for Cosmonic deployment
- kubectl - for Cosmonic deployment
- helm - for Cosmonic deployment
- wash (wasmCloud shell) - for local development
- wasmcp (MCP component tool)
- wkg (WebAssembly package manager)
- Rust toolchain

### Environment Setup

1. **Create environment configuration:**
```bash
make env-setup
# Edit .env with your values:
# - COSMONIC_LICENSE_KEY (for Cosmonic deployment)
# - GITHUB_TOKEN (with write:packages scope, for publishing)
# - GITHUB_USER (for publishing)
```

### Runtime-Specific Quick Start

#### Local Development with wash

```bash
# Build and run locally
make wash

# Access at http://localhost:8080/mcp
```

#### Kubernetes Deployment with Cosmonic

```bash
# Complete setup and deploy (handles everything automatically)
make cosmonic

# Access via NodePort shown in output
```

#### Direct wasmtime Execution

```bash
# Run with wasmtime
make wasmtime

# Note: Currently in development
```

## Management Tools

This project includes custom Rust-based management tools for different runtimes:

### wash-manager

Manages local wasmCloud development environment:
- Auto-starts wash if not running
- Manages component lifecycle
- Handles HTTP provider and link configuration
- Validates links after creation

**Location:** `tools/wash-manager/`

### cosmonic-manager

Manages Kubernetes deployment with Cosmonic Control:
- Auto-creates kind cluster if needed
- Auto-installs Cosmonic Control and HostGroup
- Generates manifests from templates
- Provides deployment endpoints

**Location:** `tools/cosmonic-manager/`

**Templates:** `manifests/templates/`
- `httptrigger.yaml.tpl` - Cosmonic HTTPTrigger deployment
- `deployment.yaml.tpl` - Standard Kubernetes deployment

## Available Make Targets

### Build Targets
- `make build` - Build all components and compose them
- `make build-components` - Build individual components
- `make compose` - Compose components into single WASM

### Runtime Targets

#### wash (Local wasmCloud)
- `make wash` - Build and run in wash runtime
- `make wash-start` - Start wash runtime
- `make wash-stop` - Stop wash runtime and clean up
- `make wash-status` - Check wash runtime status
- `make wash-clean` - Clean up wash configurations and links

#### Cosmonic (Kubernetes)
- `make cosmonic` - Build and deploy to Cosmonic
- `make cosmonic-setup` - Set up cluster and install Cosmonic Control
- `make cosmonic-deploy` - Deploy to Cosmonic cluster
- `make cosmonic-status` - Check Cosmonic deployment status
- `make cosmonic-clean` - Clean up Cosmonic deployment

#### wasmtime (Direct WASM)
- `make wasmtime` - Run with wasmtime runtime (in development)

### Publishing
- `make publish VERSION=x.x.x` - Publish composed WASM to ghcr.io

### Setup & Validation
- `make setup` - Complete Cosmonic setup (cluster + Cosmonic Control)
- `make env-setup` - Create .env file from template
- `make validate` - Validate environment and dependencies

### Testing
- `make test` - Test the deployed MCP server (Cosmonic)

### Cleanup
- `make cleanup` - Interactive cleanup
- `make cleanup-all` - Clean everything (cluster, registry, artifacts)
- `make cleanup-cluster` - Delete only the kind cluster
- `make clean` - Clean build artifacts only

### Manager Tools
- `make wash-manager` - Build the wash-manager tool
- `make cosmonic-manager` - Build the cosmonic-manager tool

## Project Structure

```
compose-example/
├── components/              # MCP tool components
│   ├── calculator/         # Math operations component
│   ├── string-utils/       # String manipulation component
│   └── system-info/        # System utilities component
├── tools/                  # Management tools
│   ├── wash-manager/       # wash runtime manager (Rust)
│   └── cosmonic-manager/   # Cosmonic deployment manager (Rust)
├── scripts/                # Legacy automation scripts (preserved)
│   ├── setup-kind.sh
│   ├── install-cosmonic.sh
│   ├── build-and-push.sh
│   ├── deploy.sh
│   ├── test-mcp.sh
│   └── cleanup.sh
├── manifests/
│   ├── templates/          # Manifest templates (Tera format)
│   │   ├── httptrigger.yaml.tpl
│   │   └── deployment.yaml.tpl
│   ├── httptrigger.yaml    # Generated HTTPTrigger manifest
│   └── deployment.yaml     # Generated Deployment manifest
├── build/                  # Build artifacts
│   └── mcp-multi-tools.wasm
├── config/                 # Configuration files
│   ├── .env.example       # Environment template
│   └── .env               # Local configuration (gitignored)
├── Makefile               # Automation targets
└── README.md              # This file
```

## Configuration

### Environment Variables

Create `.env` from the template:

```bash
# GitHub Configuration (for publishing)
GITHUB_TOKEN=your_github_token_here
GITHUB_USER=your_github_username

# Cosmonic Configuration (for Kubernetes deployment)
COSMONIC_LICENSE_KEY=your_cosmonic_license_key_here

# Deployment Configuration
VERSION=0.2.0
CLUSTER_NAME=cosmonic-cluster
NAMESPACE=default
APP_NAME=mcp-multi-tools
```

### Component Configuration

Each component has a `wkg-config.toml`:
```toml
[namespace_registries]
wasmcp = "ghcr.io"
```

## Runtime Behaviors

### wash Runtime (Local Development)

**Workflow:**
1. `make wash` builds components and starts wash-manager
2. wash-manager checks if wash is running
3. If not running, starts wash with `WASMCLOUD_MAX_CORE_INSTANCES_PER_COMPONENT=50`
4. Creates/validates HTTP server config on port 8080
5. Stops existing component if running (for hot reload)
6. Starts component
7. Starts HTTP provider if needed
8. Creates link between provider and component
9. Validates link creation

**Access:** `http://localhost:8080/mcp`

**Features:**
- Component restart handling (stops existing before starting new)
- Persistent configs and links between wash restarts
- Link validation to prevent race conditions
- Clean shutdown with `make wash-stop`

**Note:** To deploy code changes, run `make wash` again - it will rebuild and restart the component.

### Cosmonic Runtime (Kubernetes)

**Workflow:**
1. `make cosmonic` builds components and runs cosmonic-manager deploy
2. cosmonic-manager checks prerequisites:
   - Cluster connectivity
   - Cosmonic Control CRDs
3. If prerequisites missing, auto-runs setup:
   - Creates kind cluster with NodePort mappings
   - Installs Cosmonic Control (helm)
   - Installs HostGroup
4. Renders manifest from template (`manifests/templates/`)
5. Writes generated manifest to `manifests/`
6. Applies manifest to cluster
7. Waits for deployment ready
8. Shows access information with actual NodePort

**Access:** `http://localhost:<nodeport>/mcp` (NodePort shown in deploy output)

**Features:**
- Automatic setup if cluster/Cosmonic not present
- Template-based manifest generation (no inline YAML)
- Generated manifests are committable artifacts
- HTTPTrigger with rolling updates
- 2 replica deployment

### wasmtime Runtime (In Development)

Direct WASM execution without wasmCloud or Kubernetes.

**Planned:** `wasmtime serve` execution

## Accessing the MCP Server

### wash Runtime

```bash
curl -X POST http://localhost:8080/mcp \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "capabilities": {},
      "clientInfo": {"name": "test", "version": "1.0"}
    }
  }'
```

### Cosmonic Runtime

The deploy output shows the actual NodePort:

```bash
curl -X POST http://localhost:30950/mcp \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "capabilities": {},
      "clientInfo": {"name": "test", "version": "1.0"}
    }
  }'
```

### MCP Protocol

The server implements the Model Context Protocol (JSON-RPC 2.0):

1. **Initialize** (required first call)
2. **tools/list** - Get available tools
3. **tools/call** - Execute a tool

Example tool call:
```bash
curl -X POST http://localhost:8080/mcp \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
      "name": "add",
      "arguments": {"a": 5, "b": 3}
    }
  }'
```

## Development Workflow

### Building and Testing Locally

```bash
# Build components
make build

# Run in wash for local testing
make wash

# Test the server
curl -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'

# Stop when done
make wash-stop
```

### Deploying to Kubernetes

```bash
# Deploy (auto-handles setup if needed)
make cosmonic

# Check status
make cosmonic-status

# View logs
kubectl logs -l app=mcp-multi-tools -n default

# Clean up
make cosmonic-clean
```

### Publishing

```bash
# Publish to ghcr.io
make publish VERSION=0.2.0
```

## Troubleshooting

### wash Runtime Issues

```bash
# Check wash status
make wash-status

# Clean up persistent state
make wash-clean

# Restart fresh
make wash-stop && make wash
```

### Cosmonic Runtime Issues

```bash
# Check deployment status
make cosmonic-status

# View logs
kubectl logs -l app=mcp-multi-tools -n default

# Re-run setup
make cosmonic-setup

# Clean and redeploy
make cosmonic-clean && make cosmonic
```

### Common Issues

1. **License Key Not Set**
   - Set `COSMONIC_LICENSE_KEY` in `.env`
   - Get a license from https://cosmonic.com

2. **Port 8080 Already in Use (wash)**
   - Check what's using the port: `lsof -i :8080`
   - Stop conflicting service or change `DEV_PORT` in Makefile

3. **Build Failures**
   - Run `make validate` to check tools
   - Ensure `wkg-config.toml` exists in components

4. **Cosmonic Prerequisites Missing**
   - cosmonic-manager auto-runs setup if needed
   - Requires `COSMONIC_LICENSE_KEY` in environment

## Additional Resources

- [wasmcp Documentation](https://github.com/cosmonic/wasmcp)
- [Cosmonic Documentation](https://cosmonic.com/docs)
- [MCP Specification](https://modelcontextprotocol.io)
- [WebAssembly Component Model](https://component-model.bytecodealliance.org/)
- [wasmCloud Documentation](https://wasmcloud.com/docs)

## License

This project is for demonstration purposes. See individual component licenses for details.

## Contributing

Contributions are welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test thoroughly with both runtimes
5. Submit a pull request

## Security

- Never commit `.env` or any file with secrets
- Use environment variables for sensitive data
- Rotate tokens and keys regularly
- Review `.gitignore` before committing
