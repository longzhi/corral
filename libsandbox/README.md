# libsandbox - C Interposition Library

Lightweight userspace sandbox library for Corral that intercepts libc calls and enforces file/network/exec policies.

## Overview

libsandbox provides application-level sandboxing by intercepting system calls through:
- **macOS**: DYLD_INSERT_LIBRARIES interposition
- **Linux**: LD_PRELOAD mechanism

The library checks operations against a policy (loaded from environment variables) and optionally communicates with the Corral broker for complex decisions.

## Architecture

```
┌─────────────────────────────────────────┐
│         Sandboxed Process              │
│                                         │
│  App calls libc → open("/etc/passwd")  │
│         ↓                               │
│  libsandbox intercepts                  │
│         ↓                               │
│  Policy check (local or broker)         │
│         ↓                               │
│  ALLOW → real open()                    │
│  DENY  → errno=EACCES, return -1        │
└─────────────────────────────────────────┘
```

## Intercepted Functions

### File Operations
- `open()` / `openat()` - File opening (read/write detection)
- `access()` - File accessibility check
- `stat()` / `lstat()` - File status
- `unlink()` - File deletion
- `rename()` - File renaming

### Network Operations
- `connect()` - Outbound connections
- `bind()` - Binding to ports
- `getaddrinfo()` - DNS resolution

### Process Operations
- `execve()` - Execute program
- `posix_spawn()` - Spawn process

### Dynamic Library
- `dlopen()` - Load dynamic library

## Policy Format

Policies are JSON documents specifying allowed operations:

```json
{
  "fs": {
    "read": [
      "/tmp/*",
      "/home/user/data/**",
      "/etc/passwd"
    ],
    "write": [
      "/tmp/*",
      "/home/user/data/**"
    ]
  },
  "network": {
    "allow": [
      "127.0.0.1:*",
      "api.example.com:443",
      "*.cdn.example.com:*"
    ]
  },
  "exec": [
    "/bin/ls",
    "/usr/bin/*"
  ]
}
```

### Path Patterns

- `*` - Matches any characters except `/`
- `**` - Matches any characters including `/` (recursive)
- Exact paths work as expected

### Policy Loading

Policies are loaded from environment variables (checked in order):

1. `SANDBOX_POLICY` - Inline JSON string
2. `SANDBOX_POLICY_FILE` - Path to JSON file

If no policy is found, everything is denied by default.

## Broker Communication

For complex decisions, the library can communicate with the Corral broker via Unix socket:

- Set `SANDBOX_SOCKET` env var to socket path
- Library sends JSON-RPC 2.0 requests
- Broker responds with allow/deny decisions

Currently, the library makes all decisions locally for performance. Broker integration is optional and can be used for audit logging or dynamic policy updates.

## Building

```bash
# Build for current platform
make

# Build for specific platform
make macos   # → libsandbox.dylib
make linux   # → libsandbox.so

# Build and run tests
make test

# Clean
make clean
```

### Build Requirements

- C compiler (gcc or clang)
- pthread support
- fnmatch (POSIX)
- cJSON (included)

## Testing

```bash
cd tests
make
./run_tests.sh
```

The test suite includes:
- File access tests (read/write to allowed/denied paths)
- Network connection tests
- DNS resolution tests
- Process execution tests

## Usage

### macOS

```bash
SANDBOX_POLICY='{"fs":{"read":["/tmp/*"],"write":["/tmp/*"]},"network":{"allow":[]},"exec":[]}' \
DYLD_FORCE_FLAT_NAMESPACE=1 \
DYLD_INSERT_LIBRARIES=/path/to/libsandbox.dylib \
./your_program
```

### Linux

```bash
SANDBOX_POLICY='{"fs":{"read":["/tmp/*"],"write":["/tmp/*"]},"network":{"allow":[]},"exec":[]}' \
LD_PRELOAD=/path/to/libsandbox.so \
./your_program
```

### Policy from File

```bash
echo '{"fs":{"read":["/tmp/*"],"write":["/tmp/*"]}}' > /tmp/policy.json
SANDBOX_POLICY_FILE=/tmp/policy.json \
DYLD_FORCE_FLAT_NAMESPACE=1 \
DYLD_INSERT_LIBRARIES=/path/to/libsandbox.dylib \
./your_program
```

## Integration with Corral

The Rust platform modules (`platform/macos.rs` and `platform/linux.rs`) set up the environment:

```rust
// macOS example
let dylib_path = sandbox_dir.join("libsandbox.dylib");
cmd.env("DYLD_FORCE_FLAT_NAMESPACE", "1");
cmd.env("DYLD_INSERT_LIBRARIES", dylib_path);
cmd.env("SANDBOX_POLICY", policy_json);
cmd.env("SANDBOX_SOCKET", broker_socket_path);
```

## Thread Safety

- Policy is loaded once at library init and is read-only thereafter
- Mutex protection on broker socket communication
- Safe for multi-threaded applications

## Memory Safety

- No leaks in policy parsing (all allocated memory freed on cleanup)
- Constructor/destructor attributes ensure proper init/cleanup
- All string arrays properly freed

## Limitations

### What It Can Block

✅ Interpreted scripts (Python, Node, Bash) - they use libc  
✅ Most compiled programs - they use libc for I/O  
✅ Programs using standard library functions  

### What It Cannot Block

❌ Direct syscalls (bypassing libc)  
❌ Malicious compiled binaries designed to evade userspace interposition  
❌ Kernel-level operations  

**Threat Model**: Designed for untrusted community scripts, not for adversarial compiled binaries. For stronger isolation on Linux, use bubblewrap (bwrap) instead.

## Performance

- Local policy checks: ~few microseconds (fnmatch on small lists)
- No broker overhead for simple path/network checks
- Policy loaded once at init, cached in memory
- Zero overhead for non-sandboxed processes

## Code Structure

```
libsandbox/
├── policy.h          - Policy engine interface
├── policy.c          - Policy parsing, path/network matching (274 lines)
├── comm.h            - Broker communication interface
├── comm.c            - JSON-RPC over Unix socket (187 lines)
├── interpose_macos.c - macOS DYLD interpose impl (401 lines)
├── interpose_linux.c - Linux LD_PRELOAD impl (413 lines)
├── cJSON.h/cJSON.c   - JSON parser (external, MIT license)
├── Makefile          - Build system
├── tests/            - Test programs and scripts
└── README.md         - This file

Total: ~1,300 lines of C (excluding cJSON)
```

## License

Same as Corral project (see top-level LICENSE).

## Credits

- cJSON by Dave Gamble (https://github.com/DaveGamble/cJSON)
- Inspired by various sandboxing projects (Firejail, Bubblewrap, etc.)
