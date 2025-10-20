#!/bin/bash
set -euo pipefail

# Script to clean up kind cluster and associated resources

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "${SCRIPT_DIR}")"

# Load environment variables if .env exists
if [ -f "${PROJECT_ROOT}/config/.env" ]; then
    set -a
    source "${PROJECT_ROOT}/config/.env"
    set +a
fi

CLUSTER_NAME="${KIND_CLUSTER_NAME:-cosmonic-cluster}"
REGISTRY_NAME='kind-registry'

echo "=== Cleanup Script for Cosmonic MCP Deployment ==="
echo ""

# Function to confirm dangerous operations
confirm_action() {
    local action="$1"

    # Check if running in non-interactive mode
    if [ "${FORCE_CLEANUP:-}" = "true" ] || [ "${CLEANUP_MODE:-}" = "auto" ]; then
        echo "Auto-confirming: ${action}"
        return 0
    fi

    echo "WARNING: About to ${action}"
    read -p "Are you sure? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Skipping: ${action}"
        return 1
    fi
    return 0
}

# Check what needs cleaning
echo "Checking for resources to clean..."
echo ""

# Check for kind cluster
if kind get clusters 2>/dev/null | grep -q "^${CLUSTER_NAME}$"; then
    echo "✓ Found kind cluster: ${CLUSTER_NAME}"
    FOUND_CLUSTER=true
else
    echo "✗ No kind cluster found: ${CLUSTER_NAME}"
    FOUND_CLUSTER=false
fi

# Check for Docker registry
if docker ps -a --format '{{.Names}}' | grep -q "^${REGISTRY_NAME}$"; then
    echo "✓ Found Docker registry: ${REGISTRY_NAME}"
    FOUND_REGISTRY=true
else
    echo "✗ No Docker registry found: ${REGISTRY_NAME}"
    FOUND_REGISTRY=false
fi

# Check for dangling WASM images in registry
if [ "$FOUND_REGISTRY" = true ]; then
    REGISTRY_IMAGES=$(docker exec ${REGISTRY_NAME} ls /var/lib/registry/docker/registry/v2/repositories 2>/dev/null | wc -l 2>/dev/null || echo "0")
    REGISTRY_IMAGES=$(echo "$REGISTRY_IMAGES" | tr -d ' ' | head -1)
    if [ "$REGISTRY_IMAGES" -gt 0 ]; then
        echo "✓ Found ${REGISTRY_IMAGES} repositories in registry"
    fi
fi

echo ""

# If nothing to clean, exit
if [ "$FOUND_CLUSTER" = false ] && [ "$FOUND_REGISTRY" = false ]; then
    echo "Nothing to clean up. Exiting."
    exit 0
fi

# Handle automated cleanup mode
if [ -n "${CLEANUP_TARGET:-}" ]; then
    # Automated mode: use CLEANUP_TARGET environment variable
    case "${CLEANUP_TARGET}" in
        all|everything)
            OPTION=1
            echo "Auto-selected: Clean everything (cluster + registry)"
            ;;
        cluster)
            OPTION=2
            echo "Auto-selected: Clean kind cluster only"
            ;;
        registry)
            OPTION=3
            echo "Auto-selected: Clean Docker registry only"
            ;;
        *)
            echo "Error: Invalid CLEANUP_TARGET: ${CLEANUP_TARGET}"
            echo "Valid options: all, cluster, registry"
            exit 1
            ;;
    esac
else
    # Interactive mode: prompt for selection
    echo "=== Cleanup Options ==="
    echo "1) Clean everything (cluster + registry)"
    echo "2) Clean kind cluster only"
    echo "3) Clean Docker registry only"
    echo "4) Cancel"
    echo ""
    read -p "Select option [1-4]: " -n 1 -r OPTION
    echo
    echo ""
fi

case $OPTION in
    1)
        # Clean everything
        if [ "$FOUND_CLUSTER" = true ]; then
            if confirm_action "delete kind cluster '${CLUSTER_NAME}'"; then
                echo "Deleting kind cluster..."
                kind delete cluster --name "${CLUSTER_NAME}"
                echo "✓ Cluster deleted"
            fi
        fi

        if [ "$FOUND_REGISTRY" = true ]; then
            if confirm_action "delete Docker registry '${REGISTRY_NAME}'"; then
                echo "Stopping and removing Docker registry..."
                docker stop "${REGISTRY_NAME}" 2>/dev/null || true
                docker rm "${REGISTRY_NAME}" 2>/dev/null || true
                echo "✓ Registry deleted"
            fi
        fi
        ;;

    2)
        # Clean cluster only
        if [ "$FOUND_CLUSTER" = true ]; then
            if confirm_action "delete kind cluster '${CLUSTER_NAME}'"; then
                echo "Deleting kind cluster..."
                kind delete cluster --name "${CLUSTER_NAME}"
                echo "✓ Cluster deleted"
            fi
        else
            echo "No cluster to delete"
        fi
        ;;

    3)
        # Clean registry only
        if [ "$FOUND_REGISTRY" = true ]; then
            if confirm_action "delete Docker registry '${REGISTRY_NAME}'"; then
                echo "Stopping and removing Docker registry..."
                docker stop "${REGISTRY_NAME}" 2>/dev/null || true
                docker rm "${REGISTRY_NAME}" 2>/dev/null || true
                echo "✓ Registry deleted"
            fi
        else
            echo "No registry to delete"
        fi
        ;;

    4)
        echo "Cleanup cancelled."
        exit 0
        ;;

    *)
        echo "Invalid option. Exiting."
        exit 1
        ;;
esac

echo ""
echo "=== Cleanup complete! ==="

# Show remaining resources
echo ""
echo "Remaining resources:"
KIND_CLUSTERS=$(kind get clusters 2>/dev/null | wc -l || echo 0)
DOCKER_REGISTRIES=$(docker ps -a --format '{{.Names}}' | grep -c registry || echo 0)

echo "- Kind clusters: ${KIND_CLUSTERS}"
echo "- Docker registries: ${DOCKER_REGISTRIES}"

# Check if kubectl context needs updating
if [ "$FOUND_CLUSTER" = true ] && [ "$OPTION" != "3" ] && [ "$OPTION" != "4" ]; then
    echo ""
    echo "Note: Your kubectl context may need to be updated."
    CURRENT_CONTEXT=$(kubectl config current-context 2>/dev/null || echo "none")
    if [[ "$CURRENT_CONTEXT" == "kind-${CLUSTER_NAME}" ]]; then
        echo "WARNING: Current kubectl context was pointing to deleted cluster!"
        echo "Available contexts:"
        kubectl config get-contexts -o name
    fi
fi