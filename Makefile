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
	@echo ""
	@echo "Local Development (wash):"
	@echo "  make wash             - Build and run locally"
	@echo "  make wash-stop        - Stop local runtime"
	@echo ""
	@echo "Kubernetes Deployment (Cosmonic):"
	@echo "  make cosmonic         - Build and deploy to cluster (auto-setup)"
	@echo "  make cosmonic-status  - Check deployment status"
	@echo "  make cosmonic-clean   - Clean up deployment"
	@echo ""
	@echo "Publishing:"
	@echo "  make publish VERSION=x.x.x - Publish to ghcr.io"

# Default target
.DEFAULT_GOAL := help

# === Runtime Configuration ===

# Tool paths (auto-built as needed)
WASH_MANAGER := ./tools/wash-manager/target/release/wash-manager
COSMONIC_MANAGER := ./tools/cosmonic-manager/target/release/cosmonic-manager

# Component configuration (internal)
COMPONENT_PATH := $(PWD)/build/mcp-multi-tools.wasm
COMPONENT_ID := mcp-multi-tools

# Local development (wash runtime - override in .env)
DEV_PORT ?= 8080

# Cluster configuration (override in .env)
CLUSTER_NAME ?= cosmonic-cluster
NAMESPACE ?= default

# Publishing configuration (override in .env)
GITHUB_USER ?= $(shell git config --get user.name)
VERSION ?= 0.2.0
APP_NAME ?= mcp-multi-tools

# Container image configuration (override in .env)
# IMAGE_BASE: Repository without tag (e.g., ghcr.io/user/repo)
# IMAGE: Full reference including tag (overrides IMAGE_BASE:VERSION)
IMAGE_BASE ?= ghcr.io/wasmcp/example-mcp
IMAGE ?=

# Load environment overrides from .env if it exists
-include .env
export

# Backward compatibility: support old GHCR_REPO variable name
ifdef GHCR_REPO
IMAGE_BASE := $(GHCR_REPO)
endif

# === Setup Targets ===

.PHONY: setup
setup: cosmonic-setup ## Complete setup: cluster + Cosmonic

.PHONY: cosmonic-setup
cosmonic-setup: cosmonic-manager check-license ## Set up kind cluster and install Cosmonic Control
	@$(COSMONIC_MANAGER) setup --cluster $(CLUSTER_NAME) --license-key $(COSMONIC_LICENSE_KEY)

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
	@mkdir -p build
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
	@wasmcp compose math strings sysinfo --output $(PWD)/build/mcp-multi-tools.wasm
	@echo "✓ Composed using aliases: math strings sysinfo"

.PHONY: compose-with-profile
compose-with-profile: profile-create ## Compose using profile
	@echo "Composing with profile..."
	@wasmcp compose multi-tools --force --output $(PWD)/build/mcp-multi-tools.wasm  
	@echo "✓ Composed using profile: multi-tools"

# === Manager Tools ===

.PHONY: wash-manager
wash-manager: ## Build the wash-manager tool
	@echo "Building wash-manager..."
	@cargo build --release --manifest-path tools/wash-manager/Cargo.toml

.PHONY: cosmonic-manager
cosmonic-manager: ## Build the cosmonic-manager tool
	@echo "Building cosmonic-manager..."
	@cargo build --release --manifest-path tools/cosmonic-manager/Cargo.toml

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

.PHONY: cosmonic
cosmonic: cosmonic-deploy ## Build and deploy to Cosmonic (kind cluster)

.PHONY: cosmonic-deploy
cosmonic-deploy: cosmonic-manager ## Deploy to Cosmonic cluster
	@if [ -n "$(IMAGE)" ]; then \
		$(COSMONIC_MANAGER) deploy --image "$(IMAGE)" --namespace $(NAMESPACE) --app-name $(APP_NAME); \
	else \
		$(COSMONIC_MANAGER) deploy --image-base $(IMAGE_BASE) --version $(VERSION) --namespace $(NAMESPACE) --app-name $(APP_NAME); \
	fi

.PHONY: cosmonic-status
cosmonic-status: cosmonic-manager ## Check Cosmonic deployment status
	@$(COSMONIC_MANAGER) status --namespace $(NAMESPACE) --app-name $(APP_NAME)

.PHONY: cosmonic-clean
cosmonic-clean: cosmonic-manager ## Clean up Cosmonic deployment
	@$(COSMONIC_MANAGER) clean --namespace $(NAMESPACE) --app-name $(APP_NAME)

# === Wasmtime Runtime Targets (TODO) ===

.PHONY: wasmtime
wasmtime: build ## Build and run with wasmtime runtime
	@wasmtime serve --addr 0.0.0.0:8081 -Scli $(COMPONENT_PATH)

# === wasmcp Registry/Profile Targets ===

.PHONY: registry-setup
registry-setup: ## Set up wasmcp registry with component aliases
	@echo "Setting up wasmcp registry aliases..."
	@wasmcp registry component add math $(PWD)/components/math/build/math_s.wasm
	@wasmcp registry component add strings $(PWD)/components/string-utils/build/string_utils_s.wasm
	@wasmcp registry component add sysinfo $(PWD)/components/system-info/build/system_info_s.wasm
	@wasmcp registry component add stats $(PWD)/components/statistics/build/statistics_s.wasm
	@wasmcp registry component add pythag $(PWD)/components/pythagorean-middleware/build/pythagorean_middleware_s.wasm
	@wasmcp registry component add distance $(PWD)/components/distance-calculator/build/distance_calculator_s.wasm
	@wasmcp registry component add variance $(PWD)/components/variance-middleware/build/variance_middleware_s.wasm
	@wasmcp registry component add stddev $(PWD)/components/stddev-middleware/build/stddev_middleware_s.wasm
	@echo "✓ Aliases created: math, strings, sysinfo, stats, pythag, distance, variance, stddev"
	@wasmcp registry component list

.PHONY: profile-create
profile-create: registry-setup ## Create wasmcp profiles for common use cases
	@echo "Creating wasmcp profiles..."
	@echo ""
	@echo "Creating 'math' profile (math tools + middleware)..."
	@wasmcp registry profile add math \
		--output math-profile.wasm \
		pythag distance math
	@echo ""
	@echo "Creating 'stats' profile (statistics suite)..."
	@wasmcp registry profile add stats \
		--output stats-profile.wasm \
		stddev variance stats math
	@echo ""
	@echo "Creating 'multi-tools' profile (general purpose)..."
	@wasmcp registry profile add multi-tools \
		--output multi-tools-profile.wasm \
		math strings sysinfo
	@echo ""
	@echo "✓ Profiles created successfully!"
	@echo ""
	@echo "Available profiles:"
	@echo "  • math        - Math operations (basic + pythagorean + distance)"
	@echo "  • stats       - Statistics suite (mean, variance, stddev)"
	@echo "  • multi-tools - General purpose (math + strings + sysinfo)"
	@wasmcp registry profile list

# === Publishing Targets ===

.PHONY: publish
publish: build check-github ## Publish composed WASM to ghcr.io with VERSION
	@if [ -z "$(VERSION)" ]; then \
		echo "Error: VERSION is required. Usage: make publish VERSION=0.2.0"; \
		exit 1; \
	fi
	$(eval PACKAGE_IMAGE := $(IMAGE_BASE):$(VERSION))
	$(eval REPO_URL := https://github.com/$(GITHUB_USER)/compose-example)
	@echo "Publishing to $(PACKAGE_IMAGE)..."
	@wkg oci push "$(PACKAGE_IMAGE)" "$(PWD)/build/mcp-multi-tools.wasm" \
		--annotation org.opencontainers.image.source="$(REPO_URL)"
	@echo "✓ Published: $(PACKAGE_IMAGE)"
	@echo "✓ Linked to repository: $(REPO_URL)"

.PHONY: check-github
check-github:
	@if [ -z "$(GITHUB_USER)" ] || [ -z "$(GITHUB_TOKEN)" ]; then \
		echo "Error: GITHUB_USER and GITHUB_TOKEN must be set"; \
		echo "Please set them in config/.env or export them"; \
		exit 1; \
	fi

# === Legacy/Alias Targets ===

.PHONY: deploy
deploy: cosmonic ## Alias for cosmonic (backward compatibility)

.PHONY: deploy-only
deploy-only: cosmonic-deploy ## Alias for cosmonic-deploy (backward compatibility)

# === Status Targets ===

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

# === Utility Targets ===

.PHONY: env-setup
env-setup: ## Create .env file from template
	@if [ ! -f .env ]; then \
		echo "Creating .env from template..."; \
		cp .env.example .env; \
		echo "✓ Created .env"; \
		echo "Please edit .env and set your values"; \
	else \
		echo ".env already exists"; \
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

