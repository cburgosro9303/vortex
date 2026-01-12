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

- **Git Backend**
  - Clone and fetch Git repositories
  - Branch, tag, and commit checkout support
  - Spring Cloud Config file conventions (`{app}.yml`, `{app}-{profile}.yml`)
  - Background refresh with configurable intervals
  - Exponential backoff on failures
  - Async operations with `tokio`

### Planned

- Additional backends (S3, PostgreSQL)
- Property-level access control (PLAC)
- Real-time updates via WebSocket
- Feature flags support
- Encryption/decryption

## Quick Start

### Prerequisites

- Rust 1.92+ (edition 2024)
- Cargo
- Git (for Git backend)

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

### Using the Git Backend

```rust
use vortex_git::{GitBackend, GitBackendConfig, ConfigSource, ConfigQuery};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the Git backend
    let config = GitBackendConfig::builder()
        .uri("https://github.com/your-org/config-repo.git")
        .local_path("/tmp/config-repo")
        .default_label("main")
        .search_paths(vec!["config"])
        .build()?;

    // Create the backend (clones repository if needed)
    let backend = GitBackend::new(config).await?;

    // Fetch configuration
    let query = ConfigQuery::new("myapp", vec!["dev", "local"]);
    let result = backend.fetch(&query).await?;

    println!("Config for {}: {} property sources",
        result.name(),
        result.property_sources().len()
    );

    Ok(())
}
```

### Git Backend with Auto-Refresh

```rust
use vortex_git::{GitBackend, GitBackendConfig, RefreshConfig};
use std::time::Duration;

// Configure refresh settings
let refresh_config = RefreshConfig {
    interval: Duration::from_secs(30),
    max_failures: 3,
    backoff_multiplier: 2.0,
    max_backoff: Duration::from_secs(300),
};

// Create backend with auto-refresh enabled
let backend = GitBackend::with_auto_refresh(git_config, refresh_config).await?;

// Backend will automatically fetch updates every 30 seconds
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
  "version": "abc123def456",
  "state": null,
  "propertySources": [
    {
      "name": "git:main:myapp-dev.yml",
      "source": {
        "server.port": 8081,
        "logging.level": "DEBUG"
      }
    },
    {
      "name": "git:main:myapp.yml",
      "source": {
        "server.port": 8080,
        "spring.application.name": "myapp"
      }
    },
    {
      "name": "git:main:application.yml",
      "source": {
        "server.port": 8080,
        "management.endpoints.enabled": true
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
│   │   ├── config/         # ConfigMap, ConfigValue, PropertySource
│   │   ├── format/         # JSON, YAML, Properties serialization
│   │   ├── merge/          # Deep merge strategies
│   │   └── error.rs        # Error types
│   ├── vortex-git/         # Git backend implementation
│   │   ├── backend.rs      # GitBackend (implements ConfigSource)
│   │   ├── repository/     # Git operations (clone, fetch, checkout)
│   │   ├── reader/         # Config file parsing and resolution
│   │   ├── source/         # ConfigSource trait, ConfigQuery, ConfigResult
│   │   └── sync/           # Background refresh scheduler
│   ├── vortex-server/      # HTTP server (Axum-based)
│   │   ├── handlers/       # HTTP request handlers
│   │   ├── extractors/     # Path, query, accept extractors
│   │   ├── middleware/     # RequestId, Logging
│   │   └── response/       # Response formatters
│   └── vortex-sources/     # Configuration backends registry
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
cargo test -p vortex-git

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
│                  ConfigSource Trait                      │
│              (Abstraction for backends)                  │
└─────────────────────────┬───────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
          ▼               ▼               ▼
┌─────────────────┐ ┌───────────┐ ┌───────────────┐
│   GitBackend    │ │ S3Backend │ │ DBBackend     │
│   (vortex-git)  │ │  (future) │ │   (future)    │
└─────────────────┘ └───────────┘ └───────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────┐
│                   Content Negotiation                    │
│         (JSON / YAML / Properties based on Accept)       │
└─────────────────────────────────────────────────────────┘
```

## Git Backend Configuration

The Git backend supports the following configuration options:

| Option | Description | Default |
|--------|-------------|---------|
| `uri` | Git repository URL (HTTPS or SSH) | Required |
| `local_path` | Local path for cloned repository | Required |
| `default_label` | Default branch/tag when not specified | `main` |
| `search_paths` | Directories to search for config files | Root |
| `clone_timeout` | Timeout for clone operations | 120s |
| `fetch_timeout` | Timeout for fetch operations | 30s |
| `force_pull` | Force pull on existing repository | `false` |
| `username` | Username for HTTPS auth | None |
| `password` | Password/token for HTTPS auth | None |

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

### File Resolution Order

The Git backend resolves configuration files in the following order (highest priority first):

1. `{application}-{profile}.yml` - App + profile specific
2. `{application}.yml` - App specific
3. `application-{profile}.yml` - Profile specific base
4. `application.yml` - Base configuration

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
