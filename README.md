# Vortex Config

[![CI](https://github.com/iumotion/vortex-config/actions/workflows/ci.yml/badge.svg)](https://github.com/iumotion/vortex-config/actions/workflows/ci.yml)

A high-performance, cloud-native configuration server written in Rust. Designed as a drop-in replacement for Spring Cloud Config with enhanced features.

## Features

- 🚀 **High Performance**: Cold start < 500ms, memory footprint < 30MB
- 🔐 **Property-Level Access Control (PLAC)**: Fine-grained security at the property level
- 📦 **Multiple Backends**: Git, S3, SQL (PostgreSQL, MySQL, SQLite)
- 🎯 **Spring Cloud Config Compatible**: Drop-in replacement for existing Spring Boot clients
- 🔄 **Real-time Updates**: WebSocket push with semantic diff
- 🏷️ **Native Feature Flags**: Built-in feature flag support with targeting
- ✅ **Compliance Engine**: PCI-DSS, SOC2 validation built-in

## Quick Start

### Prerequisites

- Rust 1.92 or later
- Cargo

### Build

```bash
# Clone the repository
git clone https://github.com/iumotion/vortex-config.git
cd vortex-config

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run linting
cargo clippy --workspace -- -D warnings
```

### Development Commands

The project includes convenient aliases in `.cargo/config.toml`:

```bash
cargo c      # Check all crates
cargo b      # Build all crates
cargo t      # Test all crates
cargo lint   # Run clippy with warnings as errors
cargo release # Build release version
```

## Project Structure

```
vortex-config/
├── crates/
│   ├── vortex-core/      # Core domain types and traits
│   ├── vortex-server/    # HTTP server (Axum-based)
│   └── vortex-sources/   # Configuration backends
├── .github/
│   └── workflows/
│       └── ci.yml        # CI pipeline
├── Cargo.toml            # Workspace manifest
├── rust-toolchain.toml   # Rust version pinning
├── rustfmt.toml          # Formatting rules
└── clippy.toml           # Linting configuration
```

## Contributing

1. Ensure all tests pass: `cargo test --workspace`
2. Ensure code is formatted: `cargo fmt --all`
3. Ensure no clippy warnings: `cargo clippy --workspace -- -D warnings`

## License

MIT OR Apache-2.0
