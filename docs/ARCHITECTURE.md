# Architecture Overview

This document provides an overview of Corral's architecture and component interactions.

## System Components

```
┌─────────────────────────────────────────────────────────────┐
│                      Agent Runtime                          │
│                                                             │
│  ┌──────────────┐         ┌──────────────────────────────┐ │
│  │              │         │    Sandbox Broker             │ │
│  │  Skill       │  Unix   │    (Host Process)             │ │
│  │  Script      │ Socket  │                               │ │
│  │              │◄───────►│  ┌────────────────────────┐  │ │
│  │  Sandboxed   │         │  │   Policy Engine        │  │ │
│  │  Environment │         │  │   - Manifest Parser    │  │ │
│  │              │         │  │   - Permission Check   │  │ │
│  │  - fs: ✗     │         │  │   - Scope Validation   │  │ │
│  │  - network: ✗│         │  └──────────┬─────────────┘  │ │
│  │  - system: ✗ │         │             │                │ │
│  │              │         │  ┌──────────▼─────────────┐  │ │
│  │  ┌────────┐  │         │  │   Service Router       │  │ │
│  │  │sandbox-│  │         │  └──────────┬─────────────┘  │ │
│  │  │ call   │  │         │             │                │ │
│  │  │ CLI    │  │         │  ┌──────────▼─────────────┐  │ │
│  │  └────────┘  │         │  │   Service Adapters     │  │ │
│  │              │         │  │   - Reminders          │  │ │
│  │              │         │  │   - Calendar (planned) │  │ │
│  │              │         │  │   - Browser  (planned) │  │ │
│  │              │         │  │   - Filesystem         │  │ │
│  │              │         │  │   - Network            │  │ │
│  └──────────────┘         │  └────────────────────────┘  │ │
│                           │                               │ │
│                           │  ┌────────────────────────┐  │ │
│                           │  │   Watchdog             │  │ │
│                           │  │   - Timeout Monitor    │  │ │
│                           │  │   - Memory Limits      │  │ │
│                           │  │   - Rate Limiting      │  │ │
│                           │  └────────────────────────┘  │ │
│                           │                               │ │
│                           │  ┌────────────────────────┐  │ │
│                           │  │   Audit Logger         │  │ │
│                           │  │   - JSONL Log Format   │  │ │
│                           │  │   - All Broker Calls   │  │ │
│                           │  └────────────────────────┘  │ │
│                           └───────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Request Flow

### 1. Skill Execution Lifecycle

```
┌─────────┐
│ corral  │
│  run    │
└────┬────┘
     │
     ▼
┌─────────────────┐
│ Parse Manifest  │ ← skill.yaml
│ (manifest.rs)   │
└────┬────────────┘
     │
     ▼
┌─────────────────┐
│ Check Policy    │ ← Previously approved?
│ (policy.rs)     │
└────┬────────────┘
     │
     ▼
┌─────────────────┐
│ Setup Sandbox   │ ← Platform-specific (bubblewrap/DYLD)
│ (platform/)     │
└────┬────────────┘
     │
     ▼
┌─────────────────┐
│ Start Broker    │ ← Unix socket server
│ (broker/)       │
└────┬────────────┘
     │
     ▼
┌─────────────────┐
│ Execute Script  │ ← Isolated environment
│                 │
└────┬────────────┘
     │
     ▼
┌─────────────────┐
│ Cleanup & Log   │ ← Audit trail
│ (audit.rs)      │
└─────────────────┘
```

### 2. Service Call Flow

```
Script calls:
  sandbox-call reminders.add --list Shopping --title "Milk"
       │
       ▼
┌──────────────────┐
│  sandbox-call    │
│  (Rust CLI)      │
└────┬─────────────┘
     │ JSON-RPC Request
     ▼
┌──────────────────┐
│  Unix Socket     │
└────┬─────────────┘
     │
     ▼
┌──────────────────┐
│  Broker          │
│  (jsonrpc.rs)    │
└────┬─────────────┘
     │
     ▼
┌──────────────────┐
│  Policy Engine   │ ← Check manifest permissions
│  (policy.rs)     │   - Service: reminders?
└────┬─────────────┘   - Access: readwrite?
     │                 - Scope: list "Shopping"?
     ▼
   Allowed?
     │
     ├─ No ──► Error: -32001 Permission Denied
     │
     └─ Yes
        │
        ▼
   ┌──────────────────┐
   │  Router          │ ← Route to adapter
   │  (router.rs)     │
   └────┬─────────────┘
        │
        ▼
   ┌──────────────────┐
   │  Reminders       │ ← Platform-specific
   │  Adapter         │   - macOS: Swift helper
   │  (adapters/)     │   - Linux: D-Bus/file
   └────┬─────────────┘
        │
        ▼
   ┌──────────────────┐
   │  System API      │ ← EventKit, D-Bus, etc.
   │                  │
   └────┬─────────────┘
        │
        ▼
   ┌──────────────────┐
   │  Audit Log       │ ← Record call
   │  (audit.rs)      │
   └────┬─────────────┘
        │
        ▼
   Response → Broker → socket → sandbox-call → Script
```

## Core Components

### CLI (main.rs)

**Responsibilities:**
- Command-line interface (`run`, `inspect`, `approve`)
- Orchestrates sandbox lifecycle
- Invokes platform-specific setup

**Key Functions:**
- `run_skill()` — Main execution entry point
- `inspect_skill()` — Display permissions
- `approve_skill()` — Interactive permission approval

### Manifest Parser (manifest.rs)

**Responsibilities:**
- Parse `skill.yaml` files
- Validate permission structure
- Provide typed access to permissions

**Data Structures:**
- `Manifest` — Top-level skill metadata
- `Permissions` — File, network, service permissions
- `ServicePermission` — Service-specific scopes

### Policy Engine (policy.rs)

**Responsibilities:**
- Check if operations are allowed
- Validate paths, domains, service scopes
- Default-deny enforcement

**Key Functions:**
- `check_path()` — File access validation
- `check_network()` — Domain/port validation
- `check_service()` — Service permission validation

### Broker (broker/)

**Responsibilities:**
- JSON-RPC server over Unix socket
- Request routing to adapters
- Error handling and response formatting

**Modules:**
- `mod.rs` — Main broker loop
- `jsonrpc.rs` — Protocol implementation
- `router.rs` — Method → adapter dispatch
- `handlers/` — Built-in service handlers

### Service Adapters (adapters/)

**Responsibilities:**
- Platform-specific service implementations
- Abstraction layer over system APIs
- Error handling and result formatting

**Current Adapters:**
- `reminders/macos.rs` — EventKit integration via Swift helper
- `reminders/stub.rs` — Placeholder for unsupported platforms

**Planned Adapters:**
- Calendar
- Browser
- Notifications
- Clipboard

### Platform Support (platform/)

**Responsibilities:**
- Platform-specific sandbox setup
- Process isolation
- Resource limits

**Modules:**
- `linux.rs` — bubblewrap or LD_PRELOAD
- `macos.rs` — DYLD_INSERT_LIBRARIES
- (Future) `windows.rs` — Restricted Token + Job Objects

### Watchdog (watchdog.rs)

**Responsibilities:**
- Execution timeout monitoring
- Memory usage tracking
- Rate limiting (future)

### Audit Logger (audit.rs)

**Responsibilities:**
- Log all broker calls
- JSONL format for easy parsing
- Include timing, parameters, results

**Log Format:**
```json
{
  "timestamp": "2025-02-20T14:30:00Z",
  "skill": "smart-shopping",
  "run_id": "abc123",
  "method": "reminders.add",
  "params": {"list": "Shopping", "title": "Milk"},
  "result": "success",
  "duration_ms": 45
}
```

## Platform-Specific Details

### Linux Isolation

**Primary Method: bubblewrap**
- Kernel namespace isolation
- Mount point control
- Network namespace (optional)
- No root required

**Fallback: LD_PRELOAD**
- libsandbox.so intercepts libc calls
- User-space enforcement
- Lower security guarantee

### macOS Isolation

**Method: DYLD_INSERT_LIBRARIES**
- libsandbox.dylib intercepts libc calls
- Function interposition
- User-space enforcement

**Swift Helpers:**
- Separate binaries for EventKit access
- JSON stdin/stdout communication
- Handles platform-specific frameworks

### Windows (Planned)

**Method: Restricted Token + Job Objects**
- Restricted Token for privilege reduction
- Job Objects for resource limits
- Detours DLL for API hooking

## Security Boundaries

### What's Protected

1. **File System** — Skill can only access declared paths
2. **Network** — Skill can only connect to approved domains/ports
3. **Services** — Skill can only call approved services with declared scopes
4. **Processes** — Skill can only spawn approved executables

### Known Limitations

1. **User-Space Interception (macOS/Windows)**
   - Can be bypassed by direct syscalls
   - Only effective for interpreted scripts
   - Native compiled binaries can escape

2. **Kernel-Level Isolation (Linux)**
   - bubblewrap provides stronger isolation
   - Namespace-based, harder to bypass
   - Still vulnerable to kernel exploits

3. **Threat Model**
   - Designed for untrusted community scripts
   - Not designed for APT-level adversaries
   - Focus on accidental/casual malice

## Performance Characteristics

- **Startup Overhead:** ~10-50ms (platform-dependent)
- **Broker Call Latency:** ~1-5ms per call
- **Memory Overhead:** ~5-10MB for broker process
- **CPU Overhead:** Minimal (<1% for typical workloads)

## Extension Points

### Adding New Service Adapters

1. Create adapter module in `corral/src/adapters/`
2. Implement platform-specific logic
3. Register in router (`broker/router.rs`)
4. Update manifest schema if needed
5. Add tests and documentation

### Adding Platform Support

1. Create platform module in `corral/src/platform/`
2. Implement isolation mechanism
3. Build/integrate C library if needed
4. Update CI for new platform
5. Document platform-specific requirements

## Testing Strategy

- **Unit Tests** — Individual component logic
- **Integration Tests** — End-to-end skill execution
- **Platform Tests** — Platform-specific isolation
- **Security Tests** — Permission enforcement

See [CONTRIBUTING.md](../CONTRIBUTING.md) for running tests.
