#!/bin/bash
set -euo pipefail

# Script to deploy the composed MCP server to Kubernetes

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "${SCRIPT_DIR}")"
MANIFESTS_DIR="${PROJECT_ROOT}/manifests"

# Save VERSION if already set (from command line)
OVERRIDE_VERSION="${VERSION:-}"

# Load environment variables if .env exists
if [ -f "${PROJECT_ROOT}/config/.env" ]; then
    set -a
    source "${PROJECT_ROOT}/config/.env"
    set +a
fi

# Configuration (use override VERSION if it was set)
CLUSTER_NAME="${KIND_CLUSTER_NAME:-cosmonic-cluster}"
NAMESPACE="${DEPLOY_NAMESPACE:-default}"
GITHUB_USER="${GITHUB_USER:-}"
GHCR_REPO="${GHCR_REPO:-ghcr.io/${GITHUB_USER}/mcp-multi-tools}"
VERSION="${OVERRIDE_VERSION:-${VERSION:-latest}}"
LOCAL_REGISTRY="${LOCAL_REGISTRY:-localhost:5001}"
USE_LOCAL_REGISTRY="${USE_LOCAL_REGISTRY:-false}"
APP_NAME="${APP_NAME:-mcp-multi-tools}"

echo "=== Deploying MCP Server to Kubernetes ==="
echo ""

# Validate environment
if [ -z "$GITHUB_USER" ] && [ "$USE_LOCAL_REGISTRY" != "true" ]; then
    echo "Error: GITHUB_USER is not set and USE_LOCAL_REGISTRY is not true."
    echo "Please set GITHUB_USER in config/.env or use local registry:"
    echo "  export USE_LOCAL_REGISTRY=true"
    exit 1
fi

# Check kubectl connection
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

# Test cluster connectivity
if ! kubectl get nodes &>/dev/null; then
    echo "Error: Cannot connect to cluster. Is it running?"
    exit 1
fi

# Create namespace if needed
if [ "$NAMESPACE" != "default" ]; then
    echo "Creating namespace: ${NAMESPACE}"
    kubectl create namespace "${NAMESPACE}" --dry-run=client -o yaml | kubectl apply -f -
fi

# Use the published ghcr.io package
IMAGE="ghcr.io/wasmcp/example-mcp:${VERSION}"
echo "Using published image: ${IMAGE}"

# Create/update image pull secret for ghcr.io if needed
if [ -n "${GITHUB_TOKEN:-}" ]; then
    echo "Creating image pull secret for ghcr.io..."
    kubectl create secret docker-registry ghcr-secret \
        --docker-server=ghcr.io \
        --docker-username="${GITHUB_USER}" \
        --docker-password="${GITHUB_TOKEN}" \
        --docker-email="${GITHUB_USER}@users.noreply.github.com" \
        --namespace="${NAMESPACE}" \
        --dry-run=client -o yaml | kubectl apply -f -
fi

# Check if Cosmonic CRDs are available
if kubectl get crd httptriggers.control.cosmonic.io &>/dev/null; then
    DEPLOY_TYPE="httptrigger"
    echo "✓ Cosmonic HTTPTrigger CRD found - deploying as HTTPTrigger"
else
    DEPLOY_TYPE="deployment"
    echo "⚠ Cosmonic CRDs not found - deploying as standard Deployment"
    echo "  To use HTTPTrigger, run: ./scripts/install-cosmonic.sh"
fi

# Create manifest directory
mkdir -p "${MANIFESTS_DIR}"

# Generate appropriate manifest
if [ "$DEPLOY_TYPE" = "httptrigger" ]; then
    # Generate HTTPTrigger manifest
    MANIFEST_FILE="${MANIFESTS_DIR}/httptrigger.yaml"
    cat > "${MANIFEST_FILE}" <<EOF
apiVersion: control.cosmonic.io/v1alpha1
kind: HTTPTrigger
metadata:
  name: ${APP_NAME}
  namespace: ${NAMESPACE}
  labels:
    app: ${APP_NAME}
    version: ${VERSION}
spec:
  deployPolicy: RollingUpdate
  ingress:
    host: '*'
    paths:
    - path: /
      pathType: Prefix
  replicas: 2
  template:
    spec:
      components:
      - image: ${IMAGE}
        name: ${APP_NAME}
EOF

    echo "Applying HTTPTrigger manifest..."
    kubectl apply -f "${MANIFEST_FILE}"

    # Wait for HTTPTrigger to be ready
    echo "Waiting for HTTPTrigger to be ready..."
    for i in {1..30}; do
        if kubectl get httptrigger "${APP_NAME}" -n "${NAMESPACE}" -o jsonpath='{.status.phase}' 2>/dev/null | grep -q "Running"; then
            echo "✓ HTTPTrigger is running"
            break
        fi
        echo -n "."
        sleep 2
    done

else
    # Generate standard Kubernetes Deployment
    MANIFEST_FILE="${MANIFESTS_DIR}/deployment.yaml"
    cat > "${MANIFEST_FILE}" <<EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ${APP_NAME}
  namespace: ${NAMESPACE}
  labels:
    app: ${APP_NAME}
spec:
  replicas: 2
  selector:
    matchLabels:
      app: ${APP_NAME}
  template:
    metadata:
      labels:
        app: ${APP_NAME}
    spec:
EOF

    if [ -n "${GITHUB_TOKEN:-}" ] && [ "$USE_LOCAL_REGISTRY" != "true" ]; then
        cat >> "${MANIFEST_FILE}" <<EOF
      imagePullSecrets:
        - name: ghcr-secret
EOF
    fi

    cat >> "${MANIFEST_FILE}" <<EOF
      containers:
      - name: mcp-server
        image: ${IMAGE}
        ports:
        - containerPort: 8080
          name: http
        env:
        - name: LOG_LEVEL
          value: "info"
        resources:
          limits:
            memory: "512Mi"
            cpu: "1000m"
          requests:
            memory: "128Mi"
            cpu: "100m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 10
---
apiVersion: v1
kind: Service
metadata:
  name: ${APP_NAME}
  namespace: ${NAMESPACE}
  labels:
    app: ${APP_NAME}
spec:
  type: ClusterIP
  selector:
    app: ${APP_NAME}
  ports:
  - port: 80
    targetPort: 8080
    protocol: TCP
    name: http
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: ${APP_NAME}
  namespace: ${NAMESPACE}
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
spec:
  ingressClassName: nginx
  rules:
  - http:
      paths:
      - path: /mcp
        pathType: Prefix
        backend:
          service:
            name: ${APP_NAME}
            port:
              number: 80
EOF

    echo "Applying Kubernetes manifests..."
    kubectl apply -f "${MANIFEST_FILE}"

    # Wait for deployment to be ready
    echo "Waiting for deployment to be ready..."
    kubectl rollout status deployment/"${APP_NAME}" -n "${NAMESPACE}" --timeout=60s
fi

# Show deployment status
echo ""
echo "=== Deployment Status ==="

if [ "$DEPLOY_TYPE" = "httptrigger" ]; then
    kubectl get httptrigger "${APP_NAME}" -n "${NAMESPACE}"
    echo ""
    kubectl get pods -l "app=${APP_NAME}" -n "${NAMESPACE}"
    echo ""
    kubectl get svc -l "app=${APP_NAME}" -n "${NAMESPACE}"
else
    kubectl get deployment "${APP_NAME}" -n "${NAMESPACE}"
    echo ""
    kubectl get pods -l "app=${APP_NAME}" -n "${NAMESPACE}"
    echo ""
    kubectl get svc "${APP_NAME}" -n "${NAMESPACE}"
    echo ""
    kubectl get ingress "${APP_NAME}" -n "${NAMESPACE}"
fi

echo ""
echo "=== Deployment Complete! ==="
echo ""
echo "Application: ${APP_NAME}"
echo "Namespace: ${NAMESPACE}"
echo "Image: ${IMAGE}"
echo "Type: ${DEPLOY_TYPE}"
echo ""

# Get service endpoint
if [ "$DEPLOY_TYPE" = "httptrigger" ]; then
    SERVICE=$(kubectl get svc -l "app=${APP_NAME}" -n "${NAMESPACE}" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
    if [ -n "$SERVICE" ]; then
        echo "Service: ${SERVICE}.${NAMESPACE}.svc.cluster.local"
    fi
else
    echo "Service: ${APP_NAME}.${NAMESPACE}.svc.cluster.local"
    echo "Ingress: http://localhost/mcp"
fi

echo ""
echo "To test the deployment:"
echo "  kubectl port-forward svc/${APP_NAME} 8080:80 -n ${NAMESPACE}"
echo "  Then visit: http://localhost:8080"
echo ""
echo "To view logs:"
echo "  kubectl logs -l app=${APP_NAME} -n ${NAMESPACE}"
echo ""
echo "To get pods:"
echo "  kubectl get pods -l app=${APP_NAME} -n ${NAMESPACE}"