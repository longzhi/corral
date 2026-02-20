# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-02-20

### Added

#### Core Infrastructure
- **Manifest Parser** — Parse and validate `skill.yaml` permission declarations
- **Policy Engine** — Enforce file, network, and service permissions with default-deny model
- **Sandbox Broker** — JSON-RPC server for controlled system access
- **Audit Logger** — Automatic logging of all broker calls for transparency

#### Platform Support
- **Linux** — Isolation via bubblewrap with namespace/mount/network controls
- **macOS** — Isolation via DYLD_INSERT_LIBRARIES with libsandbox.dylib
- **libsandbox** — C interposition library (~1,300 lines) for macOS/Linux
  - Intercepts: `open()`, `connect()`, `execve()`, `dlopen()`, etc.
  - JSON policy loading from environment variables
  - Thread-safe implementation

#### Broker Services
- **File System** — Read/write operations with glob pattern support
- **Network** — HTTP requests with domain/port whitelisting
- **Process Execution** — Controlled subprocess spawning
- **Environment Variables** — Filtered environment access

#### Service Adapters
- **Reminders (macOS)** — Full CRUD operations via EventKit
  - List reminders with filters (list, completion status)
  - Add reminders with title, due date, notes, priority
  - Update, complete, and delete reminders
  - List-scoped permissions
  - Swift helper binary for EventKit integration

#### SDK
- **sandbox-call CLI** — Rust-based CLI for scripts to call broker services
  - Cross-platform (macOS, Linux)
  - Simple command-line interface
  - JSON-RPC communication over Unix sockets

#### Examples
- **hello-world** — Basic skill demonstrating file access permissions
- Comprehensive `skill.yaml` examples in documentation

#### Documentation
- **README.md** — Quick start, usage examples, architecture overview
- **DESIGN.md** — Detailed technical design and architecture
- **CONTRIBUTING.md** — Development setup and contribution guidelines

### Security
- Default-deny permission model
- Path traversal protection in file system operations
- Domain validation for network requests
- Resource monitoring (timeouts, memory limits)
- Audit trail for all broker operations

### Platform Compatibility
- Rust 1.70+
- macOS 10.15+ (Catalina or later)
- Linux with bubblewrap or LD_PRELOAD support
- Windows support planned for future release

---

## [Unreleased]

### Planned
- Calendar service adapter
- Browser service adapter
- Notifications service adapter
- Clipboard service adapter
- Windows support (Restricted Token + Job Objects)
- Python SDK
- Node.js SDK
- Rate limiting
- Advanced resource controls
