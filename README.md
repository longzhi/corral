# Corral

**A capability-based sandbox for untrusted agent skills**

Corral provides fine-grained permission control for running untrusted scripts and agent skills. Instead of complete isolation, it offers **controlled capability access** through a broker—scripts declare what they need, and the system grants (or denies) access accordingly.

## Features

- 🔒 **Declarative Permissions** — Skills declare file, network, and service access in `skill.yaml`
- 🌍 **Cross-Platform** — Linux (bubblewrap), macOS (DYLD interpose), Windows (planned)
- 🛡️ **Policy Engine** — Default deny; only explicitly granted capabilities are allowed
- 📡 **Service Broker** — Controlled access to calendar, reminders, browser, notifications, clipboard
- 📝 **Audit Logs** — All broker calls automatically logged for transparency
- 🚀 **Minimal Overhead** — Rust-powered, fast startup, low memory footprint
- 🔌 **Simple SDK** — Scripts use `sandbox-call` CLI to interact with system services

## Quick Start

### Installation

Build from source:

```bash
git clone https://github.com/yourusername/corral.git
cd corral
cargo build --release
cargo install --path corral
cargo install --path sdk/sandbox-call
```

On macOS, also build the Swift helper:

```bash
cd helpers/reminders-helper-macos
make
```

### Running a Skill

```bash
# Inspect permissions first
corral inspect --skill ./my-skill

# Approve permissions (interactive)
corral approve --skill ./my-skill

# Run the skill
corral run --skill ./my-skill
```

## Library Usage

Corral can also be used as a Rust library for embedding sandboxed execution in your applications.

### Basic Example

```rust
use corral_core::{Sandbox, SandboxBuilder};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Build a sandbox with fine-grained permissions
    let sandbox = SandboxBuilder::new()
        .fs_read(["/usr/**", "/etc/**"])
        .fs_write(["/tmp/work/**"])
        .network_deny()
        .exec_allow(["python3", "bash"])
        .timeout(Duration::from_secs(30))
        .build()?;

    // Execute a command
    let result = sandbox.execute("echo 'Hello from sandbox'").await?;
    
    println!("Exit code: {}", result.exit_code);
    println!("Output: {}", result.stdout);
    
    Ok(())
}
```

### Advanced Example with Custom Permissions

```rust
use corral_core::{Permissions, PolicyEngine, Sandbox, SandboxConfig};
use std::time::Duration;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Build permissions using the builder API
    let permissions = Permissions::builder()
        .fs_read(["$DATA_DIR/**"])
        .fs_write(["$WORK_DIR/**"])
        .network_allow(["api.example.com:443", "*.cdn.example.com:443"])
        .exec_allow(["curl", "jq", "python3"])
        .env_allow(["PATH", "LANG", "HOME"])
        .build();

    // Create sandbox configuration
    let config = SandboxConfig {
        permissions: permissions.clone(),
        work_dir: std::env::temp_dir().join("my-sandbox"),
        data_dir: Some(dirs::data_dir().unwrap().join("my-app")),
        timeout: Duration::from_secs(60),
        max_memory_mb: Some(512),
        env_vars: [
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("LANG".to_string(), "en_US.UTF-8".to_string()),
        ]
        .into_iter()
        .collect(),
    };

    // Create sandbox
    let sandbox = Sandbox::new(config)?;

    // Execute multiple commands in the same sandbox
    let result1 = sandbox.execute("python3 -c 'print(2+2)'").await?;
    println!("Python result: {}", result1.stdout.trim());

    let result2 = sandbox
        .execute_with_timeout("curl https://api.example.com", Duration::from_secs(10))
        .await?;
    println!("Curl result: {}", result2.stdout);

    // Check permissions programmatically
    let policy = sandbox.policy();
    assert!(policy.check_path_read("data/config.json"));
    assert!(policy.check_network("api.example.com", 443));
    assert!(!policy.check_network("evil.com", 443));

    Ok(())
}
```

### Permission Intersection

```rust
use corral_core::Permissions;

// Define app-level permissions
let app_perms = Permissions::builder()
    .fs_read(["/usr/**", "/etc/**", "/tmp/**"])
    .network_allow(["*.example.com:443"])
    .build();

// Define user-requested permissions
let user_perms = Permissions::builder()
    .fs_read(["/usr/**", "/home/user/**"])
    .network_allow(["api.example.com:443"])
    .build();

// Compute intersection (most restrictive)
let final_perms = app_perms.intersect(&user_perms);

// final_perms will only have:
// - fs.read: ["/usr/**"]  (common to both)
// - network.allow: ["api.example.com:443"]  (matches both patterns)
```

### Using the Policy Engine Standalone

```rust
use corral_core::{PolicyEngine, Permissions};

let permissions = Permissions::builder()
    .fs_read(["*.txt", "data/**"])
    .exec_allow(["curl", "jq"])
    .build();

let policy = PolicyEngine::new(permissions);

// Boolean checks (recommended for new code)
if policy.check_path_read("readme.txt") {
    println!("Read access granted");
}

if policy.check_exec("curl") {
    println!("Can execute curl");
}

// Result-based checks (for error handling)
policy.check_file_read("sensitive.db")?;  // Returns Err if denied
policy.check_exec_result("/usr/bin/jq")?;  // Returns Err if denied
```

### Cargo Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
corral-core = { path = "path/to/corral/corral-core" }
# Or once published:
# corral-core = "0.1"
```

## Usage Examples

### Example Skill Manifest

```yaml
# skill.yaml
name: smart-shopping
version: 1.0.0
entry: ./run.sh
runtime: bash

permissions:
  fs:
    read:
      - $SKILL_DIR/**
    write:
      - $DATA_DIR/**
      
  network:
    allow:
      - api.example.com:443
      
  services:
    reminders:
      access: readwrite
      scope:
        lists: ["Shopping"]
```

### Skill Script Example

```bash
#!/bin/bash
# run.sh

# Call broker services via sandbox-call SDK
items=$(sandbox-call network.http --method GET --url "https://api.example.com/list")
sandbox-call reminders.add --list Shopping --title "Buy milk"
sandbox-call notifications.send --title "Done!" --body "List updated"
```

### Using sandbox-call

```bash
# List reminders
sandbox-call reminders.list --list Shopping

# Add a reminder
sandbox-call reminders.add \
  --list Shopping \
  --title "Buy milk" \
  --dueDate "2025-02-10T18:00:00+08:00"

# Make HTTP request
sandbox-call network.http \
  --method GET \
  --url "https://api.example.com/data"

# Send notification
sandbox-call notifications.send \
  --title "Task Complete" \
  --body "Shopping list updated"
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Agent Runtime                   │
│                                                  │
│  ┌─────────┐    ┌──────────────────────────────┐ │
│  │  Skill  │    │        Sandbox Broker         │ │
│  │ Script  │◄──►│  (Host process, full access)  │ │
│  │         │    │                               │ │
│  │ Sandbox │    │  ┌─────────┐ ┌────────────┐  │ │
│  │ - fs: ✗ │    │  │ Policy  │ │ Service    │  │ │
│  │ - net: ✗│    │  │ Engine  │ │ Adapters   │  │ │
│  │ - sys: ✗│    │  │         │ │ (Calendar, │  │ │
│  │         │    │  │         │ │ Reminders) │  │ │
│  └────┬────┘    └──────────────┬───────────────┘ │
│       │                        │                  │
│       └──── Unix Socket ───────┘                  │
└─────────────────────────────────────────────────┘
```

**How it works:**

1. **Skill runs in isolated environment** (no direct file/network/system access)
2. **All interactions go through `sandbox-call` SDK** → Broker (JSON-RPC over Unix socket)
3. **Broker checks permissions** via Policy Engine
4. **If allowed, Service Adapters execute** the operation
5. **Results returned to script**; all calls logged for audit

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for detailed component information.

## Platform Support

| Platform | Mechanism | Status |
|----------|-----------|--------|
| **macOS** | `DYLD_INSERT_LIBRARIES` (libsandbox.dylib) | ✅ Implemented |
| **Linux** | `bubblewrap` or `LD_PRELOAD` (libsandbox.so) | ✅ Implemented |
| **Windows** | Restricted Token + Job Objects + Detours | 🚧 Planned |

### libsandbox — C Interposition Layer

For macOS and Linux (when bwrap is unavailable), Corral uses **libsandbox** — a lightweight C library that intercepts libc calls to enforce file/network/exec policies.

**Features:**
- Intercepts `open()`, `connect()`, `execve()`, `dlopen()`, and more
- JSON policy loaded from environment variables
- Optional broker communication for audit
- Thread-safe (~1,300 lines of C)

Build instructions in [libsandbox/README.md](libsandbox/README.md).

## Permission Model

Skills use a **declarative permission model**—they declare what they need in `skill.yaml`, and the system enforces it at runtime.

**Permission categories:**

- **File System** — Read/write access to specific paths (glob patterns supported)
- **Network** — Allowed domains and ports
- **Services** — System services (reminders, calendar, browser, notifications, clipboard)
- **Process Execution** — Allowed executables
- **Environment** — Allowed environment variables

**Default policy:** Deny everything unless explicitly granted.

For detailed information, see [docs/PERMISSIONS.md](docs/PERMISSIONS.md).

## Security Model

**Threat Model:** Untrusted community scripts that may be buggy or malicious, but not APT-level adversaries.

**What Corral defends against:**
- Unauthorized file access
- Unexpected network connections
- Unwanted process execution
- Resource exhaustion (memory/CPU/forkbomb)
- Unauthorized system service calls

**Limitations:**
- User-space interception (macOS/Windows) can be bypassed by direct syscalls in native binaries
- Linux `bubblewrap` provides stronger kernel-level isolation
- Not designed to sandbox malicious compiled binaries—focus is on interpreter-based scripts

See [SECURITY.md](SECURITY.md) for security policy and reporting instructions.

## Documentation

- [Design Document](docs/DESIGN.md) — Architecture and technical details
- [Permissions Guide](docs/PERMISSIONS.md) — Permission model and skill.yaml format
- [Architecture Overview](docs/ARCHITECTURE.md) — Component diagram and flow
- [Contributing Guide](CONTRIBUTING.md) — Development setup and guidelines
- [Changelog](CHANGELOG.md) — Version history

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for:

- Development environment setup
- Build instructions
- Code style guidelines
- PR process
- Project roadmap

## Examples

See the [examples/](examples/) directory:

- **hello-world** — Basic skill with file access
- **network-skill** — Demonstrates network permissions and HTTP requests

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Acknowledgments

Inspired by:
- Flatpak/Firejail permission models
- Deno's capability-based security
- OpenBSD pledge/unveil
