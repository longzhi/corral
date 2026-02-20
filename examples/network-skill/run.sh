#!/bin/bash
# Network Skill Example
# Demonstrates controlled network access and HTTP requests

set -e

echo "=== Network Skill Example ==="
echo ""

# Function to make HTTP requests through sandbox-call
make_request() {
    local url="$1"
    local method="${2:-GET}"
    
    echo "→ $method $url"
    sandbox-call network.http \
        --method "$method" \
        --url "$url" \
        2>/dev/null
}

# Test 1: Simple GET request
echo "Test 1: Simple GET request"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
response=$(make_request "https://httpbin.org/json")
echo "Response:"
echo "$response" | jq '.'
echo ""

# Test 2: GET with parameters
echo "Test 2: GitHub API request"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
gh_response=$(make_request "https://api.github.com/zen")
echo "GitHub Zen:"
echo "$gh_response" | jq -r '.body'
echo ""

# Test 3: Cache the result
echo "Test 3: Caching data to persistent storage"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cache_file="$DATA_DIR/cache.json"
cache_data=$(cat <<EOF
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "data": $(echo "$gh_response" | jq -r '.body')
}
EOF
)
sandbox-call fs.write --path "$cache_file" --content "$cache_data"
echo "✓ Cached to $cache_file"
echo ""

# Test 4: Read from cache
echo "Test 4: Reading from cache"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cached=$(sandbox-call fs.read --path "$cache_file" 2>/dev/null || echo "{}")
if [ -n "$cached" ] && [ "$cached" != "{}" ]; then
    echo "Cached data:"
    echo "$cached" | jq -r '.content' | jq '.'
else
    echo "⚠ No cache found"
fi
echo ""

# Test 5: Send notification
echo "Test 5: Sending completion notification"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if sandbox-call notifications.send \
    --title "Network Skill Complete" \
    --body "Successfully fetched and cached data from GitHub API" \
    2>/dev/null; then
    echo "✓ Notification sent"
else
    echo "⚠ Notifications not available (expected on some platforms)"
fi
echo ""

# Test 6: Demonstrate permission denial
echo "Test 6: Permission enforcement (should fail)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Attempting to access denied domain (example.com)..."
if make_request "https://example.com" 2>&1 | grep -q "denied"; then
    echo "✓ Permission correctly denied for example.com"
else
    echo "⚠ Expected permission denial"
fi
echo ""

echo "=== All tests completed ==="
echo ""
echo "Summary:"
echo "  • HTTP requests work for allowed domains (httpbin.org, api.github.com)"
echo "  • Data can be cached to \$DATA_DIR"
echo "  • Notifications can be sent (platform-dependent)"
echo "  • Denied domains are blocked by the permission system"
