# Makefile for MCP Multi-Component Project
# Run 'make help' to see available targets

.PHONY: help
help: ## Show this help message
	@echo "MCP Multi-Component Project Makefile"
	@echo "====================================="
	@echo ""
	@echo "Available targets:"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "Quick Start:"
	@echo "  1. make setup         - Set up everything from scratch"
	@echo "  2. make deploy        - Build and deploy to cluster"
	@echo "  3. make test          - Test the deployment"
	@echo "  4. make cleanup       - Clean up everything"

# Default target
.DEFAULT_GOAL := help

# === Runtime Configuration ===

WASH_MANAGER := ./tools/wash-manager/target/release/wash-manager
COMPONENT_PATH := $(PWD)/build/mcp-multi-tools.wasm
COMPONENT_ID := mcp-multi-tools
DEV_PORT := 8080
CLUSTER_NAME ?= cosmonic-cluster
NAMESPACE ?= default
GITHUB_USER ?= $(shell git config --get user.name)
VERSION ?= 0.2.0

# Load .env file if it exists
-include config/.env
export

# === Setup Targets ===

.PHONY: setup
setup: cluster cosmonic ## Complete setup: cluster + Cosmonic

.PHONY: cluster
cluster: ## Set up kind cluster with registry
	@echo "Setting up kind cluster..."
	@./scripts/setup-kind.sh

.PHONY: cosmonic
cosmonic: check-license ## Install Cosmonic Control (requires COSMONIC_LICENSE_KEY)
	@echo "Installing Cosmonic Control..."
	@./scripts/install-cosmonic.sh

.PHONY: check-license
check-license:
	@if [ -z "$(COSMONIC_LICENSE_KEY)" ]; then \
		echo "Error: COSMONIC_LICENSE_KEY is not set"; \
		echo "Please set it in config/.env or export it:"; \
		echo "  export COSMONIC_LICENSE_KEY='your-license-key'"; \
		exit 1; \
	fi

# === Build Targets ===

.PHONY: build
build: build-components compose ## Build all components and compose them

.PHONY: build-components
build-components: ## Build individual components
	@echo "Building components..."
	@for component in components/*/; do \
		if [ -f "$$component/Cargo.toml" ]; then \
			echo "Building $$(basename $$component)..."; \
			(cd "$$component" && wash build) || exit 1; \
		fi \
	done

.PHONY: compose
compose: compose-with-profile ## Compose components into single WASM using profile

.PHONY: compose-with-aliases
compose-with-aliases: registry-setup ## Compose using registry aliases
	@echo "Composing with registry aliases..."
	@mkdir -p build
	@wasmcp compose calc strings sysinfo --output $(PWD)/build/mcp-alias-composed.wasm
	@echo "✓ Composed using aliases: build/mcp-alias-composed.wasm"

.PHONY: compose-with-profile
compose-with-profile: profile-create ## Compose using profile
	@echo "Composing with profile..."
	@wasmcp compose multi-tools --force --output $(PWD)/build/mcp-alias-composed.wasm  
	@echo "✓ Composed using profile: multi-tools"

# === Wash Manager Tool ===

.PHONY: wash-manager
wash-manager: ## Build the wash-manager tool
	@echo "Building wash-manager..."
	@cargo build --release --manifest-path tools/wash-manager/Cargo.toml

# === Wash Runtime Targets ===

.PHONY: wash
wash: build wash-start ## Build and run in wash

.PHONY: wash-start
wash-start: wash-manager ## Start wash runtime
	@$(WASH_MANAGER) start --component $(COMPONENT_PATH) --id $(COMPONENT_ID) --port $(DEV_PORT)

.PHONY: wash-stop
wash-stop: wash-manager ## Stop wash runtime and clean up
	@$(WASH_MANAGER) stop --id $(COMPONENT_ID) --cleanup true

.PHONY: wash-status
wash-status: wash-manager ## Check wash runtime status
	@$(WASH_MANAGER) status

.PHONY: wash-clean
wash-clean: wash-manager ## Clean up wash configurations and links
	@$(WASH_MANAGER) clean

# === Cosmonic Runtime Targets ===

.PHONY: cosmonic-deploy
cosmonic-deploy: build deploy ## Build and deploy to Cosmonic (kind cluster)

.PHONY: cosmonic-status
cosmonic-status: status-cosmonic status-deployment ## Check Cosmonic deployment status

# === Wasmtime Runtime Targets (TODO) ===

.PHONY: wasmtime
wasmtime: build ## Build and run with wasmtime runtime
	@wasmtime serve -Scli $(COMPONENT_PATH)
	@exit 1

.PHONY: registry-setup
registry-setup: ## Set up wasmcp registry with component aliases
	@echo "Setting up wasmcp registry aliases..."
	@wasmcp registry component add calc $(PWD)/components/calculator/build/calculator_s.wasm
	@wasmcp registry component add strings components/string-utils/build/string_utils_s.wasm
	@wasmcp registry component add sysinfo components/system-info/build/system_info_s.wasm
	@echo "✓ Aliases created: calc, strings, sysinfo"
	@wasmcp registry component list

.PHONY: profile-create
profile-create: registry-setup ## Create wasmcp profile for the multi-tool server
	@echo "Creating wasmcp profile 'multi-tools'..."
	@wasmcp registry profile add multi-tools \
		--output multi-tools-profile.wasm \
		calc strings sysinfo
	@echo "✓ Profile 'multi-tools' created"
	@wasmcp registry profile list



.PHONY: push
push: check-github ## Push to ghcr.io (requires GITHUB_TOKEN)
	@echo "Pushing to ghcr.io..."
	@./scripts/build-and-push.sh

.PHONY: publish
publish: build compose check-github ## Publish composed WASM to ghcr.io with VERSION
	@if [ -z "$(VERSION)" ]; then \
		echo "Error: VERSION is required. Usage: make publish VERSION=0.2.0"; \
		exit 1; \
	fi
	@echo "Publishing to ghcr.io/wasmcp/example-mcp:$(VERSION)..."
	@wkg oci push "ghcr.io/wasmcp/example-mcp:$(VERSION)" "$(PWD)/build/mcp-multi-tools.wasm" \
		--annotation org.opencontainers.image.source="https://github.com/wasmcp/compose-example"
	@echo "✓ Published: ghcr.io/wasmcp/example-mcp:$(VERSION)"

.PHONY: check-github
check-github:
	@if [ -z "$(GITHUB_USER)" ] || [ -z "$(GITHUB_TOKEN)" ]; then \
		echo "Error: GITHUB_USER and GITHUB_TOKEN must be set"; \
		echo "Please set them in config/.env or export them"; \
		exit 1; \
	fi

# === Deploy Targets ===

.PHONY: deploy
deploy: build 
	@echo "Deploying to cluster..."
	@VERSION=$(VERSION) ./scripts/deploy.sh

.PHONY: deploy-only
deploy-only: ## Deploy to cluster (without building)
	@echo "Deploying to cluster..."
	@VERSION=$(VERSION) ./scripts/deploy.sh

# === Test Targets ===

.PHONY: test
test: ## Test the deployed MCP server
	@echo "Testing deployed MCP server..."
	@./scripts/test-mcp.sh

# === Status Targets ===

.PHONY: status
status: status-cluster status-cosmonic status-deployment ## Show all status information

.PHONY: status-cluster
status-cluster: ## Show cluster status
	@echo "=== Cluster Status ==="
	@kind get clusters 2>/dev/null | grep -q "$(CLUSTER_NAME)" && \
		echo "✓ Cluster '$(CLUSTER_NAME)' is running" || \
		echo "✗ Cluster '$(CLUSTER_NAME)' is not running"
	@echo ""
	@kubectl get nodes 2>/dev/null || echo "Cannot connect to cluster"

.PHONY: status-cosmonic
status-cosmonic: ## Show Cosmonic status
	@echo "=== Cosmonic Status ==="
	@kubectl get all -n cosmonic-system 2>/dev/null || echo "Cosmonic not installed"
	@echo ""
	@kubectl get crd 2>/dev/null | grep cosmonic || echo "No Cosmonic CRDs found"

.PHONY: status-deployment
status-deployment: ## Show deployment status
	@echo "=== Deployment Status ==="
	@kubectl get all -l app=mcp-multi-tools -n $(NAMESPACE) 2>/dev/null || echo "No deployment found"

.PHONY: logs
logs: ## Show deployment logs
	@kubectl logs -l app=mcp-multi-tools -n $(NAMESPACE) --tail=100

# === Cleanup Targets ===

.PHONY: clean
clean: ## Clean build artifacts
	@echo "Cleaning build artifacts..."
	@rm -rf build/
	@rm -rf components/*/build/
	@rm -rf components/*/target/
	@rm -f config/.cosmonic-values.yaml
	@echo "✓ Build artifacts cleaned"

.PHONY: cleanup
cleanup: ## Interactive cleanup of cluster and registry
	@echo "Starting interactive cleanup..."
	@./scripts/cleanup.sh

.PHONY: cleanup-all
cleanup-all: ## Clean everything (cluster, registry, artifacts)
	@echo "Cleaning everything..."
	@CLEANUP_TARGET=all FORCE_CLEANUP=true KIND_CLUSTER_NAME=$(CLUSTER_NAME) ./scripts/cleanup.sh
	@make clean

.PHONY: cleanup-cluster
cleanup-cluster: ## Delete only the kind cluster
	@echo "Deleting cluster..."
	@CLEANUP_TARGET=cluster FORCE_CLEANUP=true KIND_CLUSTER_NAME=$(CLUSTER_NAME) ./scripts/cleanup.sh

# === Utility Targets ===

.PHONY: env-setup
env-setup: ## Create .env file from template
	@if [ ! -f config/.env ]; then \
		echo "Creating config/.env from template..."; \
		cp config/.env.example config/.env; \
		echo "✓ Created config/.env"; \
		echo "Please edit config/.env and set your values"; \
	else \
		echo "config/.env already exists"; \
	fi

.PHONY: validate
validate: ## Validate environment and dependencies
	@echo "=== Validating Environment ==="
	@echo -n "Checking for kind... "
	@command -v kind > /dev/null && echo "✓" || (echo "✗ - Install from https://kind.sigs.k8s.io"; exit 1)
	@echo -n "Checking for kubectl... "
	@command -v kubectl > /dev/null && echo "✓" || (echo "✗ - Install kubectl"; exit 1)
	@echo -n "Checking for helm... "
	@command -v helm > /dev/null && echo "✓" || (echo "✗ - Install from https://helm.sh"; exit 1)
	@echo -n "Checking for wash... "
	@command -v wash > /dev/null && echo "✓" || (echo "✗ - Install wasmCloud tools"; exit 1)
	@echo -n "Checking for wasmcp... "
	@command -v wasmcp > /dev/null && echo "✓" || (echo "✗ - Install wasmcp"; exit 1)
	@echo -n "Checking for wkg... "
	@command -v wkg > /dev/null && echo "✓" || (echo "✗ - Install wkg"; exit 1)
	@echo -n "Checking for Docker... "
	@docker version > /dev/null 2>&1 && echo "✓" || (echo "✗ - Docker not running"; exit 1)
	@echo ""
	@echo "✓ All dependencies validated!"

