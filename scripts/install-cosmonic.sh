#!/bin/bash
set -euo pipefail

# Script to install Cosmonic Control in kind cluster

CLUSTER_NAME="${KIND_CLUSTER_NAME:-cosmonic-cluster}"
NAMESPACE="${COSMONIC_NAMESPACE:-cosmonic-system}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "${SCRIPT_DIR}")"

# Load environment variables if .env exists
if [ -f "${PROJECT_ROOT}/config/.env" ]; then
    set -a
    source "${PROJECT_ROOT}/config/.env"
    set +a
fi

echo "=== Installing Cosmonic Control in cluster: ${CLUSTER_NAME} ==="

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    echo "Error: kubectl is not installed."
    exit 1
fi

# Check if helm is available
if ! command -v helm &> /dev/null; then
    echo "Error: helm is not installed. Please install helm first."
    echo "Visit: https://helm.sh/docs/intro/install/"
    exit 1
fi

# Ensure we're using the right context
EXPECTED_CONTEXT="kind-${CLUSTER_NAME}"
CURRENT_CONTEXT=$(kubectl config current-context 2>/dev/null || echo "none")

if [ "$CURRENT_CONTEXT" != "$EXPECTED_CONTEXT" ]; then
    echo "Switching to context: ${EXPECTED_CONTEXT}"
    if kubectl config get-contexts -o name | grep -q "^${EXPECTED_CONTEXT}$"; then
        kubectl config use-context "${EXPECTED_CONTEXT}"
    else
        echo "Error: Context '${EXPECTED_CONTEXT}' not found. Is the cluster running?"
        echo "Run: ./scripts/setup-kind.sh"
        exit 1
    fi
fi

# Check cluster connectivity
echo "Checking cluster connectivity..."
if ! kubectl get nodes &>/dev/null; then
    echo "Error: Cannot connect to cluster. Is the cluster running?"
    exit 1
fi

# Create namespace
echo "Creating namespace: ${NAMESPACE}"
kubectl create namespace "${NAMESPACE}" --dry-run=client -o yaml | kubectl apply -f -

# No need to add repo - Cosmonic uses OCI registry directly
echo "Using Cosmonic OCI registry..."

# Check for license key
if [ -z "${COSMONIC_LICENSE_KEY:-}" ]; then
    echo ""
    echo "ERROR: COSMONIC_LICENSE_KEY environment variable is not set."
    echo ""
    echo "To obtain a license key:"
    echo "  1. Visit https://cosmonic.com"
    echo "  2. Sign up or log in to your account"
    echo "  3. Navigate to your account settings"
    echo "  4. Generate or copy your license key"
    echo ""
    echo "Then set it in your environment:"
    echo "  export COSMONIC_LICENSE_KEY='your-license-key-here'"
    echo ""
    echo "Or create a .env file in the project root with:"
    echo "  COSMONIC_LICENSE_KEY=your-license-key-here"
    echo ""
    exit 1
fi

# No values file needed - passing options directly to helm

echo "Installing Cosmonic Control..."
helm install cosmonic-control oci://ghcr.io/cosmonic/cosmonic-control \
    --version 0.3.0 \
    --namespace "${NAMESPACE}" \
    --set cosmonicLicenseKey="${COSMONIC_LICENSE_KEY}" \
    --set envoy.service.type=NodePort \
    --set envoy.service.httpNodePort=30950 \
    --wait \
    --timeout 5m

echo "Waiting for Cosmonic Control to be ready..."

# Wait for CRDs to be available
echo "Waiting for CRDs..."
for crd in workloads.core.cosmonic.com httptriggers.core.cosmonic.com; do
    kubectl wait --for=condition=Established crd/${crd} --timeout=60s || {
        echo "Warning: CRD ${crd} not ready yet"
    }
done

# Wait for deployments to be ready
echo "Waiting for deployments to be ready..."
echo "(Note: Container images may take several minutes to pull from ghcr.io)"

# Poll for deployment readiness
for i in {1..60}; do
    READY_COUNT=$(kubectl get deployments -n "${NAMESPACE}" -o json | jq '[.items[] | select(.status.conditions[]? | select(.type=="Available" and .status=="True"))] | length')
    TOTAL_COUNT=$(kubectl get deployments -n "${NAMESPACE}" -o json | jq '.items | length')

    if [ "$READY_COUNT" -eq "$TOTAL_COUNT" ] && [ "$TOTAL_COUNT" -gt 0 ]; then
        echo ""
        echo "✓ All deployments ready: ${READY_COUNT}/${TOTAL_COUNT}"
        break
    fi

    # Show progress every 10 iterations
    if [ $((i % 10)) -eq 0 ]; then
        echo ""
        echo "  Status: ${READY_COUNT}/${TOTAL_COUNT} deployments ready"
        kubectl get pods -n "${NAMESPACE}" --no-headers | awk '{printf "  %-40s %s\n", $1, $2}'
    else
        echo -n "."
    fi

    sleep 5
done

# Install HostGroup to run workloads
echo ""
echo "Installing HostGroup..."
helm install hostgroup oci://ghcr.io/cosmonic/cosmonic-control-hostgroup \
    --version 0.3.0 \
    --namespace "${NAMESPACE}" \
    --wait \
    --timeout 1m || {
        echo "Warning: HostGroup installation may have issues"
    }

# Wait for hosts to be ready
echo "Waiting for hosts to be ready..."
for i in {1..30}; do
    if kubectl get hosts -A 2>/dev/null | grep -q "True"; then
        echo "✓ Hosts are ready"
        kubectl get hosts -A
        break
    fi
    echo -n "."
    sleep 2
done

# Show status
echo ""
echo "=== Cosmonic Control Installation Status ==="
kubectl get all -n "${NAMESPACE}"

echo ""
echo "=== Available CRDs ==="
kubectl get crd | grep cosmonic || echo "No Cosmonic CRDs found yet"

echo ""
echo "=== Installation complete! ==="
echo ""
echo "Cosmonic Control has been installed in namespace: ${NAMESPACE}"
echo ""
echo "To verify the installation:"
echo "  kubectl get all -n ${NAMESPACE}"
echo ""
echo "To check CRDs:"
echo "  kubectl get crd | grep cosmonic"
echo ""
echo "To deploy a workload:"
echo "  kubectl apply -f manifests/httptrigger.yaml"
echo ""

# Check if there are any pods in error state
PROBLEM_PODS=$(kubectl get pods -n "${NAMESPACE}" --field-selector=status.phase!=Running,status.phase!=Succeeded -o name 2>/dev/null | wc -l)
if [ "${PROBLEM_PODS}" -gt 0 ]; then
    echo "WARNING: Some pods may not be running correctly."
    echo "Check pod status with:"
    echo "  kubectl get pods -n ${NAMESPACE}"
    echo "  kubectl describe pods -n ${NAMESPACE}"
fi