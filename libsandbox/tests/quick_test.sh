#!/bin/bash

echo "=== Quick libsandbox Test ==="

# Simple test program
cat > /tmp/test_open.c << 'EOF'
#include <stdio.h>
#include <fcntl.h>
#include <errno.h>
#include <string.h>

int main() {
    printf("Attempting to open /etc/passwd...\n");
    int fd = open("/etc/passwd", O_RDONLY);
    if (fd < 0) {
        printf("BLOCKED: %s\n", strerror(errno));
        return 0;
    }
    printf("ALLOWED (fd=%d)\n", fd);
    return 0;
}
EOF

cc -o /tmp/test_open /tmp/test_open.c

echo ""
echo "Test 1: Without sandbox (should succeed)"
/tmp/test_open

echo ""
echo "Test 2: With sandbox, deny policy (should be blocked)"
SANDBOX_POLICY='{"fs":{"read":[],"write":[]}}' \
DYLD_FORCE_FLAT_NAMESPACE=1 \
DYLD_INSERT_LIBRARIES=../libsandbox.dylib \
/tmp/test_open

echo ""
echo "Test 3: With sandbox, allow /etc/passwd (should succeed)"
SANDBOX_POLICY='{"fs":{"read":["/etc/passwd"],"write":[]}}' \
DYLD_FORCE_FLAT_NAMESPACE=1 \
DYLD_INSERT_LIBRARIES=../libsandbox.dylib \
/tmp/test_open

rm /tmp/test_open /tmp/test_open.c
echo ""
echo "✓ Quick test complete!"
