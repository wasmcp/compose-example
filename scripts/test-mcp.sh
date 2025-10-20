#!/bin/bash
set -euo pipefail

# Script to test the deployed MCP server

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "${SCRIPT_DIR}")"

# Load environment variables if .env exists
if [ -f "${PROJECT_ROOT}/config/.env" ]; then
    set -a
    source "${PROJECT_ROOT}/config/.env"
    set +a
fi

# Configuration
CLUSTER_NAME="${KIND_CLUSTER_NAME:-cosmonic-cluster}"
NAMESPACE="${DEPLOY_NAMESPACE:-default}"
COSMONIC_NAMESPACE="${COSMONIC_NAMESPACE:-cosmonic-system}"
PORT="${LOCAL_PORT:-8080}"

echo "=== Testing MCP Server ===="
echo ""

# Ensure we're using the right context
EXPECTED_CONTEXT="kind-${CLUSTER_NAME}"
CURRENT_CONTEXT=$(kubectl config current-context 2>/dev/null || echo "none")

if [ "$CURRENT_CONTEXT" != "$EXPECTED_CONTEXT" ]; then
    echo "Switching to context: ${EXPECTED_CONTEXT}"
    if kubectl config get-contexts -o name | grep -q "^${EXPECTED_CONTEXT}$"; then
        kubectl config use-context "${EXPECTED_CONTEXT}"
    else
        echo "Error: Context '${EXPECTED_CONTEXT}' not found. Is the cluster running?"
        exit 1
    fi
fi

# Check if workloads are ready
echo "Checking workload status..."
READY_COUNT=$(kubectl get workloads -n "${NAMESPACE}" -o json | jq '.items | map(select(.status.conditions[] | select(.type=="Ready" and .status=="True"))) | length')
TOTAL_COUNT=$(kubectl get workloads -n "${NAMESPACE}" -o json | jq '.items | length')

if [ "$READY_COUNT" -eq 0 ] || [ "$TOTAL_COUNT" -eq 0 ]; then
    echo "Error: No ready workloads found"
    kubectl get workloads -n "${NAMESPACE}"
    exit 1
fi

echo "✓ ${READY_COUNT}/${TOTAL_COUNT} workloads ready"

# Verify NodePort service is available
echo ""
echo "Checking Cosmonic ingress service..."
NODEPORT=$(kubectl get svc ingress -n "${COSMONIC_NAMESPACE}" -o jsonpath='{.spec.ports[?(@.port==80)].nodePort}')
if [ -z "$NODEPORT" ]; then
    echo "Error: Could not find NodePort for ingress service"
    kubectl get svc -n "${COSMONIC_NAMESPACE}"
    exit 1
fi

echo "✓ Ingress service available on NodePort: ${NODEPORT}"

# Use the NodePort directly (exposed through kind)
MCP_URL="http://localhost:${NODEPORT}/mcp"

# Test connection
echo ""
echo "Testing MCP server..."

# Create test request file
cat > /tmp/mcp-test-request.json <<EOF
{
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
}
EOF

echo "Sending initialize request to MCP server..."
echo ""

# Use python to make the HTTP request (available on all systems)
RESPONSE=$(python3 -c "
import json
import urllib.request
import urllib.error

url = '${MCP_URL}'
headers = {
    'Content-Type': 'application/json',
    'Accept': 'application/json, text/event-stream'
}

with open('/tmp/mcp-test-request.json', 'r') as f:
    data = f.read().encode('utf-8')

req = urllib.request.Request(url, data=data, headers=headers)

try:
    response = urllib.request.urlopen(req, timeout=10)
    print(response.read().decode('utf-8'))
except urllib.error.HTTPError as e:
    print(f'HTTP Error {e.code}: {e.read().decode(\"utf-8\")}')
except Exception as e:
    print(f'Error: {e}')
" 2>&1)

echo "Response:"
echo "${RESPONSE}" | python3 -m json.tool 2>/dev/null || echo "${RESPONSE}"

# Check if initialization was successful
if echo "${RESPONSE}" | grep -q '"method":"initialize"' || echo "${RESPONSE}" | grep -q '"serverInfo"'; then
    echo ""
    echo "✓ MCP server initialized successfully!"

    # List available tools first
    echo ""
    echo "Getting list of available tools..."

    cat > /tmp/mcp-list-tools.json <<EOF
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list"
}
EOF

    TOOLS_RESPONSE=$(python3 -c "
import json
import urllib.request
import urllib.error

url = '${MCP_URL}'
headers = {
    'Content-Type': 'application/json',
    'Accept': 'application/json, text/event-stream'
}

with open('/tmp/mcp-list-tools.json', 'r') as f:
    data = f.read().encode('utf-8')

req = urllib.request.Request(url, data=data, headers=headers)

try:
    response = urllib.request.urlopen(req, timeout=10)
    print(response.read().decode('utf-8'))
except urllib.error.HTTPError as e:
    print(f'HTTP Error {e.code}: {e.read().decode(\"utf-8\")}')
except Exception as e:
    print(f'Error: {e}')
" 2>&1)

    echo "Available tools:"
    echo "${TOOLS_RESPONSE}" | python3 -m json.tool 2>/dev/null || echo "${TOOLS_RESPONSE}"

    # Now test a tool call
    echo ""
    echo "Testing calculator tool..."

    cat > /tmp/mcp-tool-request.json <<EOF
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "add",
    "arguments": {
      "a": 5,
      "b": 3
    }
  }
}
EOF

    TOOL_RESPONSE=$(python3 -c "
import json
import urllib.request
import urllib.error

url = '${MCP_URL}'
headers = {
    'Content-Type': 'application/json',
    'Accept': 'application/json, text/event-stream'
}

with open('/tmp/mcp-tool-request.json', 'r') as f:
    data = f.read().encode('utf-8')

req = urllib.request.Request(url, data=data, headers=headers)

try:
    response = urllib.request.urlopen(req, timeout=10)
    print(response.read().decode('utf-8'))
except urllib.error.HTTPError as e:
    print(f'HTTP Error {e.code}: {e.read().decode(\"utf-8\")}')
except Exception as e:
    print(f'Error: {e}')
" 2>&1)

    echo "Tool response:"
    echo "${TOOL_RESPONSE}" | python3 -m json.tool 2>/dev/null || echo "${TOOL_RESPONSE}"

    if echo "${TOOL_RESPONSE}" | grep -q '"result"'; then
        echo ""
        echo "✓ Tool call successful!"
    fi
else
    echo ""
    echo "⚠ Warning: MCP server may not be responding correctly"
fi

# Cleanup
echo ""
echo "Cleaning up..."
rm -f /tmp/mcp-test-request.json /tmp/mcp-tool-request.json

echo ""
echo "=== Test Complete ===="
echo ""
echo "MCP server is accessible at:"
echo "  ${MCP_URL}"
echo "  http://localhost:30950/ (responds on any path)"
echo ""
echo "To test manually with curl:"
echo "  curl -X POST ${MCP_URL} \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test-client\",\"version\":\"1.0.0\"}}}'"