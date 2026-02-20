#!/bin/bash

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=== libsandbox Test Suite ==="
echo ""

# Detect platform
if [[ "$OSTYPE" == "darwin"* ]]; then
    LIBSANDBOX="../libsandbox.dylib"
    PRELOAD_VAR="DYLD_INSERT_LIBRARIES"
    PRELOAD_FORCE="DYLD_FORCE_FLAT_NAMESPACE=1"
else
    LIBSANDBOX="../libsandbox.so"
    PRELOAD_VAR="LD_PRELOAD"
    PRELOAD_FORCE=""
fi

# Check if library exists
if [ ! -f "$LIBSANDBOX" ]; then
    echo -e "${RED}ERROR: $LIBSANDBOX not found. Run 'make' first.${NC}"
    exit 1
fi

# Build test program
echo "Building test program..."
make clean > /dev/null 2>&1
make > /dev/null 2>&1
echo -e "${GREEN}✓ Test program built${NC}"
echo ""

# Test 1: Deny all policy
echo "=== Test 1: Deny All Policy ==="
echo "Policy: No permissions granted"
POLICY='{"fs":{"read":[],"write":[]},"network":{"allow":[]},"exec":[]}'
export SANDBOX_POLICY="$POLICY"
if [[ "$OSTYPE" == "darwin"* ]]; then
    DYLD_FORCE_FLAT_NAMESPACE=1 DYLD_INSERT_LIBRARIES="$LIBSANDBOX" ./test_sandbox 2>&1 | grep -E "(BLOCKED|ALLOWED)" || true
else
    LD_PRELOAD="$LIBSANDBOX" ./test_sandbox 2>&1 | grep -E "(BLOCKED|ALLOWED)" || true
fi
echo ""

# Test 2: Allow /tmp reads and writes
echo "=== Test 2: Allow /tmp Access ==="
echo "Policy: Allow read/write to /tmp/*"
POLICY='{"fs":{"read":["/tmp/*"],"write":["/tmp/*"]},"network":{"allow":[]},"exec":[]}'
export SANDBOX_POLICY="$POLICY"
if [[ "$OSTYPE" == "darwin"* ]]; then
    DYLD_FORCE_FLAT_NAMESPACE=1 DYLD_INSERT_LIBRARIES="$LIBSANDBOX" ./test_sandbox 2>&1 | grep -E "(Testing file|BLOCKED|ALLOWED)" || true
else
    LD_PRELOAD="$LIBSANDBOX" ./test_sandbox 2>&1 | grep -E "(Testing file|BLOCKED|ALLOWED)" || true
fi
echo ""

# Test 3: Allow specific network access
echo "=== Test 3: Allow Localhost Network ==="
echo "Policy: Allow connections to 127.0.0.1"
POLICY='{"fs":{"read":[],"write":[]},"network":{"allow":["127.0.0.1:*"]},"exec":[]}'
export SANDBOX_POLICY="$POLICY"
if [[ "$OSTYPE" == "darwin"* ]]; then
    DYLD_FORCE_FLAT_NAMESPACE=1 DYLD_INSERT_LIBRARIES="$LIBSANDBOX" ./test_sandbox 2>&1 | grep -E "(Testing network|BLOCKED|ALLOWED)" || true
else
    LD_PRELOAD="$LIBSANDBOX" ./test_sandbox 2>&1 | grep -E "(Testing network|BLOCKED|ALLOWED)" || true
fi
echo ""

# Test 4: Policy from file
echo "=== Test 4: Policy from File ==="
cat > /tmp/sandbox_policy.json <<EOF
{
  "fs": {
    "read": ["/tmp/*", "/etc/passwd"],
    "write": ["/tmp/*"]
  },
  "network": {
    "allow": ["127.0.0.1:*", "*.google.com:*"]
  },
  "exec": ["/bin/ls", "/usr/bin/*"]
}
EOF
unset SANDBOX_POLICY
export SANDBOX_POLICY_FILE="/tmp/sandbox_policy.json"
echo "Policy loaded from: /tmp/sandbox_policy.json"
if [[ "$OSTYPE" == "darwin"* ]]; then
    DYLD_FORCE_FLAT_NAMESPACE=1 DYLD_INSERT_LIBRARIES="$LIBSANDBOX" ./test_sandbox 2>&1 | head -20
else
    LD_PRELOAD="$LIBSANDBOX" ./test_sandbox 2>&1 | head -20
fi
rm /tmp/sandbox_policy.json
echo ""

echo -e "${GREEN}=== All Tests Complete ===${NC}"
echo ""
echo "Note: Some tests may show 'FAILED' with connection/timeout errors."
echo "This is expected behavior - we're testing if the sandbox BLOCKS the call."
echo "Look for 'BLOCKED by sandbox' or 'Permission denied' messages."
