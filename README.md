# `wasmcp` Multi-Component Server

Example of building, composing, and deploying multiple MCP (Model Context Protocol) tool components and middelware using `wasmcp`.

## Overview

This project demonstrates:
- Building multiple MCP tool components using `wasmcp`
- Composing components into a single WASM module
- Deploying to multiple runtimes:
  - **wash** - Local wasmCloud development
  - **Cosmonic** - Kubernetes with Cosmonic Control
  - **wasmtime** - Direct WebAssembly runtime

## Components

This project includes both:
- **tool components** (providing primitive operations)
- **middleware components** (composing tools into higher-level capabilities)

**For detailed documentation on middleware/tool chaining patterns, see [components/README.md](components/README.md)**

## Quick Start

### Prerequisites

- wasmcp
- wkg (WebAssembly package manager)
- Rust toolchain

#### Cosmonic
- Docker
- kind (Kubernetes in Docker) - for Cosmonic deployment
- kubectl - for Cosmonic deployment
- helm - for Cosmonic deployment

#### wash
- wash (wasmCloud shell) - for local development
- wkg (WebAssembly package manager)
 
#### wasmtime
- wasmtime
 
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
