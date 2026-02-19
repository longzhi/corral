#!/bin/bash
set -e

echo "=== Hello World Skill ==="
echo "Running in sandboxed environment"
echo ""

# Show environment
echo "Environment:"
echo "  SKILL_DIR: $SKILL_DIR"
echo "  WORK_DIR: $WORK_DIR"
echo "  DATA_DIR: $DATA_DIR"
echo "  USER: $USER"
echo ""

# Test file operations via broker
echo "Testing file operations..."
sandbox-call fs.write path="$WORK_DIR/hello.txt" content="Hello from sandbox!"
echo "✓ Wrote to $WORK_DIR/hello.txt"

CONTENT=$(sandbox-call fs.read path="$WORK_DIR/hello.txt" | jq -r .content)
echo "✓ Read back: $CONTENT"
echo ""

# Test network via broker (if available)
echo "Testing network access..."
if sandbox-call network.http url="https://httpbin.org/json" method="GET" 2>/dev/null | jq -r .status >/dev/null; then
    echo "✓ Network access works!"
else
    echo "⚠ Network access not available (expected in sandbox)"
fi
echo ""

echo "=== Skill completed successfully ==="
