# Vortex Config

[![CI](https://github.com/cburgosro9303/vortex-config/actions/workflows/ci.yml/badge.svg)](https://github.com/cburgosro9303/vortex-config/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Polyform%20NC%201.0-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org/)

A high-performance, cloud-native configuration server written in Rust. Designed as a drop-in replacement for Spring Cloud Config Server.

## Features

### Implemented

- **Spring Cloud Config Compatible API**
  - `GET /{application}/{profile}` - Fetch config by app and profile
  - `GET /{application}/{profile}/{label}` - Fetch config with branch/tag label
  - URL-encoded label support (e.g., `feature%2Fmy-branch`)

- **Content Negotiation**
  - JSON (`application/json`) - default
  - YAML (`application/x-yaml`, `text/yaml`)
  - Properties (`text/plain`)

- **Observability**
  - Request ID middleware (`X-Request-Id` header)
  - Structured logging with tracing
  - Health endpoint (`/health`)

- **Core Types**
  - `ConfigMap` - Hierarchical configuration with dot-notation access
  - `ConfigValue` - Type-safe configuration values
  - `PropertySource` - Configuration source abstraction
  - Deep merge with configurable strategies

- **Format Support**
  - JSON serialization/deserialization
  - YAML serialization/deserialization
  - Java `.properties` format support

### Planned

- Multiple backends (Git, S3, PostgreSQL)
- Property-level access control (PLAC)
- Real-time updates via WebSocket
- Feature flags support
- Encryption/decryption

## Quick Start

### Prerequisites

- Rust 1.85+ (edition 2024)
- Cargo

### Installation

```bash
# Clone the repository
git clone https://github.com/cburgosro9303/vortex-config.git
cd vortex-config

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace
```

### Running the Server

```rust
use std::net::SocketAddr;
use vortex_server::run_server;

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    run_server(addr).await.unwrap();
}
```

### API Usage Examples

#### Fetch Configuration

```bash
# Get config for 'myapp' with 'dev' profile (JSON)
curl http://localhost:8080/myapp/dev

# Get config with specific branch/label
curl http://localhost:8080/myapp/dev/main

# Get config as YAML
curl -H "Accept: application/x-yaml" http://localhost:8080/myapp/dev

# Get config as Properties
curl -H "Accept: text/plain" http://localhost:8080/myapp/dev

# URL-encoded branch name (feature/my-feature)
curl http://localhost:8080/myapp/dev/feature%2Fmy-feature
```

#### Response Format (JSON)

```json
{
  "name": "myapp",
  "profiles": ["dev"],
  "label": "main",
  "version": null,
  "state": null,
  "propertySources": [
    {
      "name": "git:main:config/myapp.yml",
      "source": {
        "server.port": 8080,
        "spring.application.name": "myapp",
        "database.url": "jdbc:postgresql://localhost/mydb"
      }
    }
  ]
}
```

#### Health Check

```bash
curl http://localhost:8080/health
# {"status":"UP"}
```

### Using Core Types

```rust
use vortex_core::{ConfigMap, ConfigValue};

// Create a ConfigMap
let mut config = ConfigMap::new();
config.insert("server.port".to_string(), ConfigValue::Integer(8080));
config.insert("app.name".to_string(), ConfigValue::String("myapp".to_string()));

// Access with dot notation
let port = config.get("server.port");

// Serialize to JSON
let json = serde_json::to_string_pretty(&config)?;
```

## Project Structure

```
vortex-config/
├── crates/
│   ├── vortex-core/        # Core domain types and traits
│   │   ├── config/         # ConfigMap, ConfigValue
│   │   ├── format/         # JSON, YAML, Properties serialization
│   │   ├── merge/          # Deep merge strategies
│   │   └── error.rs        # Error types
│   ├── vortex-server/      # HTTP server (Axum-based)
│   │   ├── handlers/       # HTTP request handlers
│   │   ├── extractors/     # Path, query, accept extractors
│   │   ├── middleware/     # RequestId, Logging
│   │   └── response/       # Response formatters
│   └── vortex-sources/     # Configuration backends (WIP)
├── .github/workflows/      # CI pipeline
├── docs/                   # Documentation and planning
└── Cargo.toml              # Workspace manifest
```

## Development

### Commands

```bash
cargo c      # Check all crates
cargo b      # Build all crates
cargo t      # Test all crates
cargo lint   # Run clippy with warnings as errors
cargo release # Build release version
```

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p vortex-server

# With output
cargo test --workspace -- --nocapture
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Security audit
cargo audit
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    HTTP Request                          │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                  RequestId Middleware                    │
│              (Generate/Propagate X-Request-Id)           │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                   Logging Middleware                     │
│              (Structured logging with tracing)           │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                     Axum Router                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │
│  │   /health   │  │ /{app}/{p}  │  │ /{app}/{p}/{l}  │  │
│  └─────────────┘  └─────────────┘  └─────────────────┘  │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                   Content Negotiation                    │
│         (JSON / YAML / Properties based on Accept)       │
└─────────────────────────────────────────────────────────┘
```

## Spring Cloud Config Compatibility

Vortex Config is designed to be a drop-in replacement for Spring Cloud Config Server. Spring Boot applications can use the standard `spring-cloud-starter-config` client without modifications.

```yaml
# application.yml (Spring Boot client)
spring:
  application:
    name: myapp
  cloud:
    config:
      uri: http://localhost:8080
      profile: dev
      label: main
```

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Ensure tests pass: `cargo test --workspace`
4. Ensure code is formatted: `cargo fmt --all`
5. Ensure no clippy warnings: `cargo clippy --workspace -- -D warnings`
6. Commit your changes
7. Push to the branch
8. Open a Pull Request

## License

This project is licensed under the [Polyform Noncommercial License 1.0.0](LICENSE).

### What this means

- **Allowed**: Personal use, research, education, non-profit use, modification, and non-commercial distribution
- **Not allowed**: Commercial use or distribution without explicit permission from the author
- **No liability**: The author is not responsible for any damages arising from the use of this software

For commercial licensing inquiries, please contact the author ([@cburgosro9303](https://github.com/cburgosro9303)).
