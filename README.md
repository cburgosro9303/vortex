# Vortex

[![CI](https://github.com/cburgosro9303/vortex/actions/workflows/ci.yml/badge.svg)](https://github.com/cburgosro9303/vortex/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.93.0-orange.svg)](https://www.rust-lang.org/)
[![Platforms](https://img.shields.io/badge/Platforms-Linux%20%7C%20macOS%20%7C%20Windows-green.svg)](#supported-platforms)

A fast, privacy-first desktop API client built with Rust and Slint.

## Features

- **HTTP Client** - Full HTTP/1.1 and HTTP/2 support with `reqwest` + `rustls`
- **Request Body** - JSON, Form URL-Encoded, Multipart, Raw, Binary, GraphQL
- **Authentication** - Basic, Bearer, API Key (header/query), OAuth 2.0
- **Environments** - Variable substitution with secret separation
- **Collections** - Organize requests into folders with drag-and-drop
- **Postman Import** - Import collections and environments from Postman JSON
- **Code Generation** - Export requests to 13 languages (cURL, Python, JavaScript, Rust, Go, Java, C#, PHP, Ruby, Swift, Kotlin, Dart, PowerShell)
- **Export** - HAR and OpenAPI 3.0 export
- **Testing** - Assertion-based test suites with status, header, body, and JSON path checks
- **Scripting** - Pre-request and post-response scripts
- **TLS Configuration** - Client certificates, custom CA, TLS version pinning
- **Proxy Support** - HTTP/HTTPS/SOCKS5 proxy with per-request or global configuration
- **Request History** - Automatic history with search and replay
- **Cookie Management** - Automatic cookie jar with manual override
- **WebSocket** - WebSocket connection support
- **Themes** - Light and dark mode with font scaling
- **Privacy** - Zero telemetry, zero cloud sync, all data stays local
- **Fast** - Native binary, sub-second startup, low memory footprint

## Architecture

Vortex follows a hexagonal (ports & adapters) architecture:

```
  +-------+     +-------------+     +------------------+     +--------+
  |  UI   | --> | Application | --> | Infrastructure   | --> | Domain |
  | Slint |     | Use Cases   |     | HTTP, Persistence|     | Types  |
  +-------+     +-------------+     +------------------+     +--------+
```

All dependencies point inward. Domain has zero I/O dependencies. Infrastructure implements the ports defined by Application.

## Quick Start

### Download

Download the latest binary for your platform from [Releases](https://github.com/cburgosro9303/vortex/releases).

### Build from Source

**Prerequisites:**

- Rust 1.93.0+
- Linux: `sudo apt-get install -y libfontconfig1-dev libfreetype6-dev`
- macOS: `xcode-select --install`
- Windows: Visual Studio C++ Build Tools

**Build and run:**

```bash
cargo build --workspace
cargo run -p vortex
```

**Development commands:**

```bash
cargo test --workspace        # Run all tests
cargo fmt --all -- --check    # Check formatting
cargo clippy --workspace --all-targets -- -D warnings  # Lint
```

## Supported Platforms

| Platform | Architecture | Binary |
|----------|-------------|--------|
| Linux | x86_64 | `vortex-linux-x86_64` |
| macOS | ARM64 (Apple Silicon) | `vortex-macos-aarch64` |
| macOS | x86_64 (Intel) | `vortex-macos-x86_64` |
| Windows | x86_64 | `vortex-windows-x86_64.exe` |

## Vortex vs Postman

| | Vortex | Postman |
|---|---|---|
| **Memory** | ~50 MB | ~500 MB+ (Electron) |
| **Privacy** | Zero telemetry, local-only | Cloud sync, telemetry |
| **Startup** | < 1 second | 5-10 seconds |
| **Version Control** | JSON files, git-friendly | Proprietary format |
| **Cost** | Free, open source | Freemium, paid features |
| **Telemetry** | None | Extensive |
| **Offline** | Fully offline | Requires login |

## Workspace Structure

| Crate | Description |
|-------|-------------|
| `crates/domain` | Core business types (request, response, auth, collection, environment, etc.) |
| `crates/application` | Use cases and port definitions (traits) |
| `crates/infrastructure` | Adapters: HTTP client, file persistence, import/export, code generation |
| `crates/ui` | Slint UI components and view models |
| `crates/app` | Binary entry point and dependency wiring |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

[MIT](LICENSE)
