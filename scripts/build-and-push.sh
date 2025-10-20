#!/bin/bash
set -euo pipefail

# Script to build components, compose them, and push to ghcr.io

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "${SCRIPT_DIR}")"
COMPONENTS_DIR="${PROJECT_ROOT}/components"
BUILD_DIR="${PROJECT_ROOT}/build"

# Load environment variables if .env exists
if [ -f "${PROJECT_ROOT}/config/.env" ]; then
    echo "Loading environment variables from config/.env..."
    set -a
    source "${PROJECT_ROOT}/config/.env"
    set +a
fi

# Set defaults if not provided
GITHUB_USER="${GITHUB_USER:-}"
GITHUB_TOKEN="${GITHUB_TOKEN:-}"
GHCR_REPO="${GHCR_REPO:-ghcr.io/${GITHUB_USER}/mcp-multi-tools}"
VERSION="${VERSION:-0.1.0}"
LOCAL_REGISTRY="${LOCAL_REGISTRY:-localhost:5001}"

echo "=== Building and Pushing MCP Components ==="
echo ""

# Validate required environment variables
if [ -z "$GITHUB_USER" ]; then
    echo "Error: GITHUB_USER is not set."
    echo "Please set it in config/.env or export it:"
    echo "  export GITHUB_USER=your-github-username"
    exit 1
fi

if [ -z "$GITHUB_TOKEN" ]; then
    echo "Error: GITHUB_TOKEN is not set."
    echo "Please set it in config/.env or export it:"
    echo "  export GITHUB_TOKEN=your-github-token"
    echo ""
    echo "To create a GitHub token:"
    echo "  1. Go to https://github.com/settings/tokens"
    echo "  2. Generate a new token with 'write:packages' scope"
    exit 1
fi

# Check required tools
for tool in wash wasmcp wkg; do
    if ! command -v "$tool" &> /dev/null; then
        echo "Error: $tool is not installed."
        case $tool in
            wash)
                echo "Install wash: curl -sL https://wasmcloud.com/install.sh | bash"
                ;;
            wasmcp)
                echo "Install wasmcp: cargo install wasmcp --git https://github.com/cosmonic/wasmcp"
                ;;
            wkg)
                echo "Install wkg: cargo install wkg"
                ;;
        esac
        exit 1
    fi
done

# Create build directory
mkdir -p "${BUILD_DIR}"

# Step 1: Build individual components
echo "=== Building Components ==="
BUILT_COMPONENTS=()

for component_dir in "${COMPONENTS_DIR}"/*/; do
    if [ -d "$component_dir" ] && [ -f "${component_dir}/Cargo.toml" ]; then
        component_name=$(basename "$component_dir")
        echo ""
        echo "Building component: ${component_name}"

        cd "$component_dir"

        # Build with wash (using local wkg-config.toml)
        if WKG_CONFIG_FILE=wkg-config.toml wash build; then
            # Find the built wasm file (wash builds to target/wasm32-wasip2/debug/)
            # Convert hyphens to underscores for the actual file name
            wasm_filename="${component_name//-/_}.wasm"
            wasm_file="target/wasm32-wasip2/debug/${wasm_filename}"
            if [ -f "$wasm_file" ]; then
                echo "✓ Built: ${wasm_file}"
                # Copy to build directory with original component name
                cp "$wasm_file" "${BUILD_DIR}/${component_name}.wasm"
                BUILT_COMPONENTS+=("${component_name}")
            else
                echo "Error: No wasm file found after building ${component_name}"
                echo "Expected: ${wasm_file}"
                exit 1
            fi
        else
            echo "Error: Failed to build ${component_name}"
            exit 1
        fi
    fi
done

cd "${PROJECT_ROOT}"

if [ ${#BUILT_COMPONENTS[@]} -eq 0 ]; then
    echo "Error: No components were built"
    exit 1
fi

echo ""
echo "✓ Built ${#BUILT_COMPONENTS[@]} components: ${BUILT_COMPONENTS[*]}"

# Step 2: Compose components
echo ""
echo "=== Composing Components ==="

# Build compose command
COMPOSE_CMD="wasmcp compose"
for component in "${BUILT_COMPONENTS[@]}"; do
    COMPOSE_CMD="${COMPOSE_CMD} ${BUILD_DIR}/${component}.wasm"
done
COMPOSE_CMD="${COMPOSE_CMD} --output ${BUILD_DIR}/mcp-multi-tools.wasm --force"

echo "Running: ${COMPOSE_CMD}"
if eval "${COMPOSE_CMD}"; then
    echo "✓ Successfully composed components into: ${BUILD_DIR}/mcp-multi-tools.wasm"
else
    echo "Error: Failed to compose components"
    exit 1
fi

# Verify the composed file
if [ ! -f "${BUILD_DIR}/mcp-multi-tools.wasm" ]; then
    echo "Error: Composed WASM file not found"
    exit 1
fi

WASM_SIZE=$(du -h "${BUILD_DIR}/mcp-multi-tools.wasm" | cut -f1)
echo "Composed WASM size: ${WASM_SIZE}"

# Step 3: Push to registries
echo ""
echo "=== Pushing to Registries ==="

# Login to ghcr.io
echo "Logging in to ghcr.io..."
echo "${GITHUB_TOKEN}" | wkg login ghcr.io -u "${GITHUB_USER}" --password-stdin || {
    echo "Error: Failed to login to ghcr.io"
    echo "Please check your GITHUB_TOKEN and ensure it has 'write:packages' scope"
    exit 1
}

# Push to ghcr.io using wkg oci push with repository annotation
PACKAGE_IMAGE="ghcr.io/wasmcp/example-mcp:${VERSION}"
REPO_URL="https://github.com/wasmcp/compose-example"

echo "Publishing to ${PACKAGE_IMAGE}..."
if wkg oci push "${PACKAGE_IMAGE}" "${BUILD_DIR}/mcp-multi-tools.wasm" \
    --annotation org.opencontainers.image.source="${REPO_URL}"; then
    echo "✓ Published: ${PACKAGE_IMAGE}"
    echo "✓ Linked to repository: ${REPO_URL}"
else
    echo "Error: Failed to publish to ghcr.io"
    echo "Make sure you have proper permissions to publish to ghcr.io/wasmcp"
    exit 1
fi

# Note: Local registry doesn't work for WASM components
# WASM components must be published to an OCI registry like ghcr.io

echo ""
echo "=== Build and Push Complete! ==="
echo ""
echo "Artifacts:"
echo "  Local: ${BUILD_DIR}/mcp-multi-tools.wasm (${WASM_SIZE})"
echo "  Published: ${PACKAGE_IMAGE}"
echo ""
echo "To deploy to Kubernetes with Cosmonic:"
echo "  1. Install Cosmonic: make cosmonic"
echo "  2. Deploy: ./scripts/deploy.sh"
echo ""
echo "To test the composed server locally:"
echo "  wasmcp run ${BUILD_DIR}/mcp-multi-tools.wasm"
echo ""
echo "To use in another project:"
echo "  wasmcp compose ${PACKAGE_IMAGE}"