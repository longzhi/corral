# Corral — Capability-Based Sandbox for Agent Skills

> Isolated execution environment with controlled access to system services for Agent Skill scripts.

## Overview

Corral provides a lightweight sandbox for executing untrusted Agent Skill scripts with fine-grained permission control. Instead of pure isolation, it offers **controlled capability access** through a broker—scripts declare what they need, and the system grants (or denies) access accordingly.

**Key Features:**

- 🔒 **Declarative Permissions** — Skills declare file, network, and service access in `skill.yaml`
- 🌍 **Cross-Platform** — Linux (bubblewrap), macOS (DYLD interpose), Windows (planned)
- 🛡️ **Policy Engine** — Default deny; only explicitly granted capabilities are allowed
- 📡 **Service Broker** — Controlled access to calendar, reminders, browser, notifications, clipboard
- 📝 **Audit Logs** — All broker calls automatically logged for transparency
- 🚀 **Minimal Overhead** — Rust-powered, fast startup, low memory footprint

## Quick Start

### Installation

```bash
cargo install --path corral
cargo install --path sdk/sandbox-call
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
items=$(sandbox-call network.http --url "https://api.example.com/list")
sandbox-call reminders.add --list Shopping --title "Buy milk"
sandbox-call notifications.send --title "Done!" --body "List updated"
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
│  │ - fs: ✗ │    │  │FS Proxy │ │ Service    │  │ │
│  │ - net: ✗│    │  │ Policy  │ │ Adapters   │  │ │
│  │ - sys: ✗│    │  │ Engine  │ │ (Calendar, │  │ │
│  │         │    │  │         │ │ Reminders) │  │ │
│  └────┬────┘    └──────────────┬───────────────┘ │
│       │                        │                  │
│       └──── Unix Socket ───────┘                  │
└─────────────────────────────────────────────────┘
```

**Flow:**
1. Script runs in isolated environment (no direct file/network/system access)
2. All interactions go through `sandbox-call` SDK → Broker (JSON-RPC over Unix socket)
3. Broker checks permissions via Policy Engine
4. If allowed, Service Adapters execute the operation
5. Results returned to script; all calls logged for audit

### Components

- **corral** — CLI runner, orchestrates sandbox setup and execution
- **Broker** — JSON-RPC server, policy enforcement, service routing
- **Policy Engine** — Manifest-driven permission checks (paths, domains, services)
- **Service Adapters** — Platform-specific implementations (EventKit on macOS, D-Bus on Linux, etc.)
- **sandbox-call** — SDK for scripts to call broker services
- **Watchdog** — Resource limits (timeout, memory, rate limiting)
- **Audit Logger** — JSONL logs of all broker calls

### Platform Isolation

| Platform | Mechanism | Status |
|----------|-----------|--------|
| **Linux** | `bubblewrap` or `LD_PRELOAD` (libsandbox.so) | ✅ Implemented |
| **macOS** | `DYLD_INSERT_LIBRARIES` (libsandbox.dylib) | ✅ Implemented |
| **Windows** | Restricted Token + Job Objects + Detours | 🚧 Planned |

#### libsandbox — C Interposition Layer

For macOS and Linux (when bwrap is unavailable), Corral uses **libsandbox** — a lightweight C library that intercepts libc calls to enforce file/network/exec policies.

**Features:**
- Intercepts `open()`, `connect()`, `execve()`, `dlopen()`, and more
- JSON policy loaded from environment variables
- Optional broker communication for audit
- Thread-safe, memory-safe (~1,300 lines of C)

**Build:**
```bash
cd libsandbox && make       # Builds libsandbox.dylib (macOS) or libsandbox.so (Linux)
cd libsandbox && make test  # Run test suite
```

See [libsandbox/README.md](libsandbox/README.md) for details.

## Development

### Build

```bash
cargo build --release
```

### Test

```bash
cargo test
cargo clippy
cargo fmt --check
```

### Project Structure

```
corral/
├── corral/               # Main CLI and broker
│   ├── src/
│   │   ├── main.rs       # CLI entry point
│   │   ├── manifest.rs   # skill.yaml parser
│   │   ├── policy.rs     # Permission engine
│   │   ├── broker/       # JSON-RPC server + routing
│   │   ├── adapters/     # Service implementations
│   │   ├── platform/     # Linux/macOS/Windows isolation
│   │   ├── watchdog.rs   # Resource monitoring
│   │   └── audit.rs      # Logging
│   └── Cargo.toml
├── sdk/
│   └── sandbox-call/     # Rust CLI SDK for scripts
└── helpers/              # Platform-specific helpers (Swift for macOS EventKit, etc.)
```

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

## Contributing

Contributions welcome! Please:

1. Read `DESIGN.md` for architecture details
2. Follow Rust conventions (rustfmt, clippy)
3. Add tests for new features
4. Update docs as needed

### Roadmap

- [x] Phase 1: Linux support + core broker + fs/network
- [x] Phase 2: macOS support + DYLD interpose
- [x] Phase 3 (partial): Reminders service adapter (macOS)
- [ ] Phase 3 (complete): Calendar, browser, notifications, clipboard adapters
- [ ] Phase 4: Windows support
- [ ] Phase 5: SDK for Python/Node
- [ ] Phase 6: Rate limiting, advanced resource controls

## System Services

### Reminders (macOS)

Corral provides controlled access to macOS Reminders via EventKit.

**Requirements:**
- macOS 10.15+ (Catalina or later)
- Swift helper binary (built automatically)
- First run will prompt for Reminders access permission

**Build the helper:**
```bash
cd helpers/reminders-helper-macos
make
```

**Usage in skill manifest:**
```yaml
permissions:
  services:
    reminders:
      access: readwrite  # or 'read' for list-only
      scope:
        lists: ["Shopping", "Work"]  # Optional: restrict to specific lists
```

**Available methods:**
- `reminders.list` — List reminders (optional filters: list, completed)
- `reminders.add` — Create a reminder (requires: title, list; optional: dueDate, notes, priority)
- `reminders.update` — Update a reminder by ID
- `reminders.complete` — Mark a reminder as completed
- `reminders.delete` — Delete a reminder

**Example:**
```bash
# List all incomplete reminders in "Shopping" list
sandbox-call reminders.list --list Shopping --completed false

# Add a reminder
sandbox-call reminders.add \
  --list Shopping \
  --title "Buy milk" \
  --dueDate "2025-02-10T18:00:00+08:00" \
  --notes "Organic preferred"

# Complete a reminder
sandbox-call reminders.complete --id "EK:xxx"
```

**Technical details:**
- Rust adapter spawns Swift helper binary
- Communication via stdin/stdout JSON
- Helper uses EventKit framework
- Adapter auto-locates helper binary (checks: `REMINDERS_HELPER_PATH` env var, binary directory, `../helpers/`)
- On unsupported platforms, returns "Service unavailable" error

## License

MIT License - see `LICENSE` file for details.

## Acknowledgments

Inspired by:
- Flatpak/Firejail permission models
- Deno's capability-based security
- OpenBSD pledge/unveil
