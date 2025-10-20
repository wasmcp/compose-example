# MCP Multi-Component Server

Example of building, composing, and deploying multiple MCP (Model Context Protocol) tool components as WebAssembly modules to Kubernetes using Cosmonic Control and wasmcp.

## 🎯 Overview

This project demonstrates:
- Building multiple MCP tool components using `wasmcp`
- Composing components into a single WASM module
- Deploying to Kubernetes with Cosmonic Control
- Using GitHub Container Registry (ghcr.io) for artifact storage

## 📦 Components

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

## 🚀 Quick Start

### Prerequisites

```bash
# Validate all dependencies
make validate
```

Required tools:
- Docker
- kind (Kubernetes in Docker)
- kubectl
- helm
- wash (wasmCloud shell)
- wasmcp (MCP component tool)
- wkg (WebAssembly package manager)

### Setup

1. **Create environment configuration:**
```bash
make env-setup
# Edit config/.env with your values:
# - GITHUB_TOKEN (with write:packages scope)
# - GITHUB_USER
# - COSMONIC_LICENSE_KEY
```

2. **Complete setup (cluster + Cosmonic):**
```bash
make setup
```

3. **Build and deploy:**
```bash
make deploy
```

4. **Test the deployment:**
```bash
make test
```

### Complete Workflow from Scratch

```bash
# 1. Clean everything if starting fresh
make cleanup-all

# 2. Set up cluster with NodePort mapping
make cluster

# 3. Install Cosmonic Control and HostGroup
make cosmonic

# 4. Build components and compose
make build

# 5. Push to ghcr.io
make push

# 6. Deploy to Kubernetes
make deploy-only

# 7. Test the MCP server
make test
```

## 📋 Available Make Targets

### Setup & Configuration
- `make setup` - Complete setup (cluster + Cosmonic)
- `make cluster` - Set up kind cluster with registry
- `make cosmonic` - Install Cosmonic Control
- `make env-setup` - Create .env file from template
- `make validate` - Validate environment and dependencies

### Build & Deploy
- `make build` - Build all components and compose them
- `make deploy` - Build, push, and deploy to cluster
- `make deploy-local` - Deploy using local registry
- `make push` - Push to ghcr.io

### Testing
- `make test` - Run all tests
- `make test-local` - Test composed server locally
- `make test-components` - Test individual components
- `make port-forward` - Port-forward the deployed service

### Status & Monitoring
- `make status` - Show all status information
- `make logs` - Show deployment logs
- `make info` - Show project information

### Cleanup
- `make cleanup` - Interactive cleanup
- `make cleanup-all` - Clean everything
- `make clean` - Clean build artifacts only

## 🏗️ Project Structure

```
compose-example/
├── components/              # MCP tool components
│   ├── calculator/         # Math operations component
│   ├── string-utils/       # String manipulation component
│   └── system-info/        # System utilities component
├── scripts/                # Automation scripts
│   ├── setup-kind.sh      # Kind cluster setup
│   ├── install-cosmonic.sh # Cosmonic installation
│   ├── build-and-push.sh  # Build and registry push
│   ├── deploy.sh          # Kubernetes deployment
│   └── cleanup.sh         # Cleanup script
├── manifests/             # Generated Kubernetes manifests
├── build/                 # Build artifacts
├── config/               # Configuration files
│   ├── .env.example      # Environment template
│   └── .env              # Local configuration (gitignored)
├── Makefile              # Automation targets
└── README.md            # This file
```

## 🔧 Configuration

### Environment Variables

Create `config/.env` from the template:

```bash
# GitHub Configuration
GITHUB_TOKEN=your_github_token_here
GITHUB_USER=your_github_username
GHCR_REPO=ghcr.io/${GITHUB_USER}/mcp-multi-tools

# Cosmonic Configuration
COSMONIC_LICENSE_KEY=your_cosmonic_license_key_here

# Deployment Configuration
VERSION=0.1.0
KIND_CLUSTER_NAME=cosmonic-cluster
DEPLOY_NAMESPACE=default
```

### Component Configuration

Each component has a `wkg-config.toml`:
```toml
[namespace_registries]
wasmcp = "ghcr.io"
```

## 🐳 Local Development

### Running Components Locally

```bash
# Build components
make build

# Test locally with wasmcp
wasmcp run build/mcp-multi-tools.wasm
```

### Testing Individual Components

```bash
# Run component tests
make test-components

# Test a specific component
cd components/calculator
cargo test
```

## 🚢 Deployment

### Deployment Architecture

The deployment uses Cosmonic Control's HTTPTrigger CRD which:
1. Pulls the WASM component from ghcr.io
2. Creates Workload resources
3. Schedules workloads on available Hosts
4. Configures ingress routing through Envoy

### Deploy to Kind Cluster

```bash
# Full deployment pipeline
make deploy

# Or step by step:
make build          # Build components
make push           # Push to registry
make deploy-only    # Deploy to cluster
```

### HTTPTrigger Manifest

The deployment creates an HTTPTrigger resource:
```yaml
apiVersion: control.cosmonic.io/v1alpha1
kind: HTTPTrigger
metadata:
  name: mcp-multi-tools
spec:
  deployPolicy: RollingUpdate
  ingress:
    host: mcp.localhost.cosmonic.sh
    paths:
    - path: /
      pathType: Prefix
  replicas: 2
  template:
    spec:
      components:
      - image: ghcr.io/wasmcp/example-mcp:0.1.0
        name: mcp-multi-tools
```

### Using Local Registry

For development without pushing to ghcr.io:

```bash
# Deploy using local registry
make deploy-local
```

### Verify Deployment

```bash
# Check status
make status

# View logs
make logs

# Test the MCP server
make test
```

## 🌐 Accessing the MCP Server

The MCP server is exposed through Cosmonic's ingress on NodePort 30950.

### MCP Protocol

The server implements the Model Context Protocol (MCP) which uses JSON-RPC 2.0:

1. **Initialize Request** (required first):
```bash
curl -X POST http://localhost:30950/mcp \
  -H 'Host: mcp.localhost.cosmonic.sh' \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "capabilities": {},
      "clientInfo": {
        "name": "test-client",
        "version": "1.0.0"
      }
    }
  }'
```

2. **Tool Call**:
```bash
curl -X POST http://localhost:30950/mcp \
  -H 'Host: mcp.localhost.cosmonic.sh' \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
      "name": "calculator_add",
      "arguments": {"a": 5, "b": 3}
    }
  }'
```

## 🧹 Cleanup

### Interactive Cleanup
```bash
make cleanup
# Choose what to clean:
# 1) Everything
# 2) Cluster only
# 3) Registry only
# 4) Cancel
```

### Quick Cleanup
```bash
# Clean everything
make cleanup-all

# Clean specific resources
make cleanup-cluster   # Delete cluster
make cleanup-registry  # Delete registry
make clean            # Clean build artifacts
```

## 🔍 Troubleshooting

### Check Environment
```bash
make env      # Show configuration
make validate # Validate dependencies
```

### Debug Deployment
```bash
# Check cluster status
make status-cluster

# Check Cosmonic status
make status-cosmonic

# Check deployment
make status-deployment

# View logs
kubectl logs -l app=mcp-multi-tools -n default

# Get shell access
make shell
```

### Common Issues

1. **License Key Not Set**
   - Set `COSMONIC_LICENSE_KEY` in `config/.env`
   - Get a license from https://cosmonic.com

2. **GitHub Token Issues**
   - Ensure token has `write:packages` scope
   - Check token expiration

3. **Build Failures**
   - Run `make validate` to check tools
   - Ensure `wkg-config.toml` exists in components

4. **Deployment Failures**
   - Check cluster is running: `make status-cluster`
   - Verify Cosmonic is installed: `make status-cosmonic`

## 📚 Additional Resources

- [wasmcp Documentation](https://github.com/cosmonic/wasmcp)
- [Cosmonic Documentation](https://cosmonic.com/docs)
- [MCP Specification](https://modelcontextprotocol.io)
- [WebAssembly Component Model](https://component-model.bytecodealliance.org/)

## 📄 License

This project is for demonstration purposes. See individual component licenses for details.

## 🤝 Contributing

Contributions are welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test thoroughly
5. Submit a pull request

## 🛡️ Security

- Never commit `config/.env` or any file with secrets
- Use environment variables for sensitive data
- Rotate tokens and keys regularly
- Review `.gitignore` before committing