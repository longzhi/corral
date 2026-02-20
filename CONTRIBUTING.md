# Contributing to Corral

Thank you for your interest in contributing to Corral! This guide will help you get started.

## Development Environment

### Prerequisites

- **Rust** (1.70+) — Install via [rustup](https://rustup.rs/)
- **Cargo** — Comes with Rust
- **Git**
- **macOS only:** Xcode Command Line Tools (`xcode-select --install`)
- **macOS only:** Swift compiler (comes with Xcode)
- **Linux only:** bubblewrap (`apt install bubblewrap` or equivalent)

### Clone the Repository

```bash
git clone https://github.com/yourusername/corral.git
cd corral
```

## Build Instructions

### Core Components (All Platforms)

```bash
# Build all Rust components
cargo build

# Build in release mode
cargo build --release

# Install binaries locally
cargo install --path corral
cargo install --path sdk/sandbox-call
```

### Platform-Specific Helpers

#### macOS: Swift Reminders Helper

```bash
cd helpers/reminders-helper-macos
make
```

This builds the Swift helper binary that interfaces with EventKit.

#### Linux: libsandbox.so

```bash
cd libsandbox
make
```

#### macOS: libsandbox.dylib

```bash
cd libsandbox
make
```

## Running Tests

### Rust Tests

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p corral
cargo test -p sandbox-call

# Run integration tests
cargo test --test integration_test
```

### libsandbox Tests

```bash
cd libsandbox
make test
```

## Code Style

### Rust

We follow standard Rust conventions:

```bash
# Format code
cargo fmt

# Check formatting without making changes
cargo fmt --check

# Run clippy (linter)
cargo clippy

# Fix clippy warnings automatically where possible
cargo clippy --fix
```

**Before submitting a PR:**

1. Run `cargo fmt` to format your code
2. Run `cargo clippy` and fix all warnings
3. Make sure `cargo test` passes
4. Add tests for new features

### Documentation

- All public APIs must have doc comments
- Use `///` for public items
- Use `//!` for module-level docs
- Include examples in doc comments where helpful

Example:

```rust
/// Checks if a path is allowed by the policy.
///
/// # Arguments
///
/// * `path` - The path to check
/// * `access` - The type of access (read/write)
///
/// # Returns
///
/// `true` if the path is allowed, `false` otherwise
///
/// # Example
///
/// ```
/// let allowed = policy.check_path("/tmp/file.txt", AccessType::Read);
/// ```
pub fn check_path(&self, path: &str, access: AccessType) -> bool {
    // Implementation
}
```

### C Code (libsandbox)

- Follow K&R style
- Use 4-space indentation
- Document functions with comments
- Keep functions short and focused

## Project Structure

```
corral/
├── corral/                   # Main CLI and broker
│   ├── src/
│   │   ├── main.rs           # CLI entry point
│   │   ├── manifest.rs       # skill.yaml parser
│   │   ├── policy.rs         # Permission engine
│   │   ├── broker/           # JSON-RPC server + routing
│   │   │   ├── mod.rs
│   │   │   ├── jsonrpc.rs
│   │   │   ├── router.rs
│   │   │   └── handlers/     # Service handlers
│   │   ├── adapters/         # Service implementations
│   │   │   ├── reminders/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── macos.rs
│   │   │   │   └── stub.rs
│   │   │   └── mod.rs
│   │   ├── platform/         # Platform-specific isolation
│   │   │   ├── linux.rs
│   │   │   ├── macos.rs
│   │   │   └── mod.rs
│   │   ├── watchdog.rs       # Resource monitoring
│   │   └── audit.rs          # Logging
│   └── Cargo.toml
├── sdk/
│   └── sandbox-call/         # CLI SDK for scripts
│       ├── src/main.rs
│       └── Cargo.toml
├── libsandbox/               # C interposition library
│   ├── src/
│   └── Makefile
├── helpers/                  # Platform-specific helpers
│   └── reminders-helper-macos/
├── examples/                 # Example skills
└── docs/                     # Documentation
```

## Pull Request Process

1. **Fork the repository** and create a new branch from `main`
2. **Make your changes** following the code style guidelines
3. **Add tests** for new functionality
4. **Update documentation** as needed
5. **Run the full test suite** and ensure everything passes
6. **Commit your changes** with clear, descriptive commit messages
7. **Push to your fork** and submit a pull request

### Commit Message Format

Use clear, descriptive commit messages:

```
Add network permission support for HTTPS

- Implement HTTPS domain validation
- Add tests for network policy engine
- Update documentation with examples
```

### PR Checklist

Before submitting:

- [ ] Code formatted with `cargo fmt`
- [ ] No warnings from `cargo clippy`
- [ ] All tests pass (`cargo test`)
- [ ] New features have tests
- [ ] Documentation updated
- [ ] CHANGELOG.md updated (if applicable)

## Development Roadmap

### ✅ Phase 1 — Complete
- Linux support with bubblewrap
- Core broker + fs/network handlers
- Basic policy engine

### ✅ Phase 2 — Complete
- macOS support with DYLD interpose
- libsandbox C library
- Platform abstraction layer

### ✅ Phase 3 — Partial
- Reminders service adapter (macOS)
- Swift helper for EventKit integration
- Service adapter framework

### 🚧 Phase 3 — In Progress
- [ ] Calendar service adapter
- [ ] Browser service adapter
- [ ] Notifications service adapter
- [ ] Clipboard service adapter

### 📋 Phase 4 — Planned
- [ ] Windows support (Restricted Token + Job Objects + Detours)
- [ ] Windows service adapters

### 📋 Phase 5 — Future
- [ ] Python SDK (`pip install corral-sdk`)
- [ ] Node.js SDK (`npm install @corral/sdk`)
- [ ] Rate limiting and advanced resource controls
- [ ] WebAssembly support

## Areas for Contribution

We welcome contributions in these areas:

### High Priority
- Windows support
- Additional service adapters (calendar, browser, notifications, clipboard)
- Python and Node.js SDKs
- More comprehensive tests

### Medium Priority
- Performance optimizations
- Better error messages
- Documentation improvements
- More example skills

### Low Priority
- Additional platform support (FreeBSD, etc.)
- GUI for permission management
- Skill marketplace/registry

## Getting Help

- **Issues:** Open an issue on GitHub for bugs or feature requests
- **Discussions:** Use GitHub Discussions for questions and ideas
- **Documentation:** Check the [docs/](docs/) directory

## Code of Conduct

Be respectful and constructive. We're all here to build something useful together.

## License

By contributing to Corral, you agree that your contributions will be licensed under the MIT License.
