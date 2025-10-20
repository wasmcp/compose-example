#!/bin/bash
set -euo pipefail

# Script to set up a kind cluster for Cosmonic deployment

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "${SCRIPT_DIR}")"

# Load environment variables if .env exists
if [ -f "${PROJECT_ROOT}/config/.env" ]; then
    set -a
    source "${PROJECT_ROOT}/config/.env"
    set +a
fi

CLUSTER_NAME="${KIND_CLUSTER_NAME:-cosmonic-cluster}"

echo "=== Setting up kind cluster: ${CLUSTER_NAME} ==="

# Check if kind is installed
if ! command -v kind &> /dev/null; then
    echo "Error: kind is not installed. Please install kind first."
    echo "Visit: https://kind.sigs.k8s.io/docs/user/quick-start/#installation"
    exit 1
fi

# Check if cluster already exists
if kind get clusters 2>/dev/null | grep -q "^${CLUSTER_NAME}$"; then
    echo "Cluster '${CLUSTER_NAME}' already exists."
    read -p "Delete and recreate? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Deleting existing cluster..."
        kind delete cluster --name "${CLUSTER_NAME}"
    else
        echo "Using existing cluster."
        kubectl config use-context "kind-${CLUSTER_NAME}"
        exit 0
    fi
fi

# Create kind cluster with registry
echo "Creating kind cluster with local registry..."
cat <<EOF | kind create cluster --name "${CLUSTER_NAME}" --config -
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
  kubeadmConfigPatches:
  - |
    kind: InitConfiguration
    nodeRegistration:
      kubeletExtraArgs:
        node-labels: "ingress-ready=true"
  extraPortMappings:
  - containerPort: 80
    hostPort: 80
    protocol: TCP
  - containerPort: 443
    hostPort: 443
    protocol: TCP
  - containerPort: 30950
    hostPort: 30950
    protocol: TCP
    listenAddress: "127.0.0.1"
- role: worker
- role: worker
containerdConfigPatches:
- |-
  [plugins."io.containerd.grpc.v1.cri".registry]
    config_path = "/etc/containerd/certs.d"
EOF

# Set up a local Docker registry
REGISTRY_NAME='kind-registry'
REGISTRY_PORT='5001'

# Check if registry already exists
if [ "$(docker inspect -f '{{.State.Running}}' "${REGISTRY_NAME}" 2>/dev/null || true)" != 'true' ]; then
    echo "Creating local Docker registry..."
    docker run \
        -d --restart=always -p "127.0.0.1:${REGISTRY_PORT}:5000" \
        --network bridge --name "${REGISTRY_NAME}" \
        registry:2
else
    echo "Local registry already running."
fi

# Connect the registry to the kind network if not already connected
if [ "$(docker inspect -f='{{json .NetworkSettings.Networks.kind}}' "${REGISTRY_NAME}")" = 'null' ]; then
    echo "Connecting registry to kind network..."
    docker network connect "kind" "${REGISTRY_NAME}"
fi

# Document the local registry
cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: ConfigMap
metadata:
  name: local-registry-hosting
  namespace: kube-public
data:
  localRegistryHosting.v1: |
    host: "localhost:${REGISTRY_PORT}"
    help: "https://kind.sigs.k8s.io/docs/user/local-registry/"
EOF

# Set up registry configuration for kind nodes
for node in $(kind get nodes --name "${CLUSTER_NAME}"); do
  echo "Configuring registry for node: ${node}"
  docker exec "${node}" mkdir -p /etc/containerd/certs.d/localhost:${REGISTRY_PORT}
  cat <<EOF | docker exec -i "${node}" tee /etc/containerd/certs.d/localhost:${REGISTRY_PORT}/hosts.toml
[host."http://kind-registry:5000"]
EOF
done

# Install NGINX Ingress Controller
echo "Installing NGINX Ingress Controller..."
kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/kind/deploy.yaml

# Wait for ingress controller to be ready
echo "Waiting for ingress controller to be ready..."
kubectl wait --namespace ingress-nginx \
  --for=condition=ready pod \
  --selector=app.kubernetes.io/component=controller \
  --timeout=60s || {
    echo "Warning: Ingress controller may still be starting up..."
    echo "You can check its status with: kubectl get pods -n ingress-nginx"
}

echo "=== Kind cluster setup complete! ==="
echo ""
echo "Cluster: ${CLUSTER_NAME}"
echo "Context: kind-${CLUSTER_NAME}"
echo "Registry: localhost:${REGISTRY_PORT}"
echo ""
echo "To use this cluster:"
echo "  kubectl config use-context kind-${CLUSTER_NAME}"
echo ""
echo "To push images to the local registry:"
echo "  docker tag myimage:latest localhost:${REGISTRY_PORT}/myimage:latest"
echo "  docker push localhost:${REGISTRY_PORT}/myimage:latest"
echo ""
echo "Then reference in Kubernetes as:"
echo "  image: localhost:${REGISTRY_PORT}/myimage:latest"