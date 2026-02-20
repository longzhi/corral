#!/bin/bash

cat > /tmp/test_debug.c << 'EOF'
#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <errno.h>
#include <string.h>

int main() {
    const char *path = "/etc/passwd";
    char *resolved = realpath(path, NULL);
    
    printf("Original: %s\n", path);
    printf("Resolved: %s\n", resolved ? resolved : "NULL");
    free(resolved);
    
    printf("\nAttempting to open %s...\n", path);
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        printf("BLOCKED: %s\n", strerror(errno));
    } else {
        printf("ALLOWED (fd=%d)\n", fd);
    }
    
    return 0;
}
EOF

cc -o /tmp/test_debug /tmp/test_debug.c

echo "=== Path Resolution Debug ==="
/tmp/test_debug

echo ""
echo "=== With wildcard policy ==="
SANDBOX_POLICY='{"fs":{"read":["/private/etc/*"],"write":[]}}' \
DYLD_FORCE_FLAT_NAMESPACE=1 \
DYLD_INSERT_LIBRARIES=../libsandbox.dylib \
/tmp/test_debug

rm /tmp/test_debug /tmp/test_debug.c
