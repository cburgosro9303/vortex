# Vortex Config

[![CI](https://github.com/cburgosro9303/vortex-config/actions/workflows/ci.yml/badge.svg)](https://github.com/cburgosro9303/vortex-config/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/cburgosro9303/vortex-config/branch/main/graph/badge.svg)](https://codecov.io/gh/cburgosro9303/vortex-config)
[![License](https://img.shields.io/badge/license-Polyform%20NC%201.0-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-50%25%20complete-yellow.svg)](STATUS.md)

A high-performance, cloud-native configuration server written in Rust. Designed as a drop-in replacement for Spring Cloud Config Server.

> **Project Status:** 50% Complete (5/10 epics) - [See Detailed Status](STATUS.md)

## 📚 Documentation

- **[Getting Started](docs/wiki/Getting-Started.md)** - Quick setup and first steps
- **[Wiki Home](docs/wiki/Home.md)** - Complete documentation hub
- **[API Reference](docs/wiki/API-Reference.md)** - Full API documentation
- **[Configuration Guide](docs/wiki/Configuration.md)** - Server configuration
- **[Deployment Guide](docs/wiki/Deployment.md)** - Production deployment
- **[Architecture](docs/wiki/Architecture.md)** - Internal architecture
- **[PRD](docs/PRD.md)** - Product requirements document

## ⚡ Key Features

### ✅ Currently Available

- **🔄 Spring Cloud Config Compatible** - Drop-in replacement, no client changes needed
- **🚀 High Performance** - Cold start < 500ms, latency p99 < 10ms, ~20MB memory
- **💾 Git Backend** - Clone, fetch, auto-refresh, branches/tags support
- **📦 Smart Cache** - Moka async cache with TTL, invalidation, metrics
- **📊 Observability** - Prometheus metrics, structured logging, tracing
- **🎨 Multiple Formats** - JSON, YAML, Java Properties
- **🐳 Production Ready** - Docker (~37MB), Kubernetes manifests, CI/CD

### 📋 Planned Features

- **Epic 6:** S3, PostgreSQL, MySQL, SQLite backends
- **Epic 7:** Property-level access control (PLAC), schema validation
- **Epic 8:** WebSocket real-time updates, semantic diff
- **Epic 9:** Feature flags, templating, compliance engine
- **Epic 10:** Canary rollouts, drift detection, multi-cluster

[View Complete Roadmap →](STATUS.md)

## 🚀 Quick Start

### With Docker (Recommended)

```bash
docker run -d \
  -p 8888:8888 \
  -e GIT_URI=https://github.com/spring-cloud-samples/config-repo.git \
  -e GIT_DEFAULT_LABEL=main \
  --name vortex-config \
  vortex-config:latest

# Test it
curl http://localhost:8888/health
curl http://localhost:8888/foo/dev | jq
```

### From Source

**Prerequisites:**
- Rust 1.92+ (edition 2024)
- Git 2.x+ installed on system

```bash
# Clone and build
git clone https://github.com/cburgosro9303/vortex-config.git
cd vortex-config
cargo build --release

# Configure
export GIT_URI=https://github.com/your-org/config-repo.git
export GIT_DEFAULT_LABEL=main

# Run
cargo run --release --bin vortex-server
```

### Example API Usage

```bash
# Get configuration (JSON)
curl http://localhost:8888/myapp/production

# Get with specific branch
curl http://localhost:8888/myapp/production/v1.0.0

# Get as YAML
curl -H "Accept: application/x-yaml" http://localhost:8888/myapp/production

# Clear cache
curl -X DELETE http://localhost:8888/cache/myapp/production

# Prometheus metrics
curl http://localhost:8888/metrics
```

**[→ Complete Getting Started Guide](docs/wiki/Getting-Started.md)**

## 🏗️ Project Structure

```
vortex-config/
├── crates/
│   ├── vortex-core/        # Domain types, ConfigMap, formats, merge
│   ├── vortex-git/         # Git backend with auto-refresh
│   ├── vortex-server/      # Axum HTTP server, cache, handlers
│   └── vortex-sources/     # Backend registry (future)
├── deployment/             # Docker, docker-compose, K8s manifests
├── docs/                   # Documentation, PRD, planning, wiki
└── .github/workflows/      # CI/CD pipeline
```

**[→ Detailed Architecture](docs/wiki/Architecture.md)**

## 👨‍💻 Development

```bash
# Build and test
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace -- -D warnings

# Run locally
cargo run --bin vortex-server
```

**[→ Complete Development Guide](docs/wiki/Development.md)**

## 🐳 Deployment

### Docker

```bash
# Build image
docker build -f deployment/Dockerfile -t vortex-config:latest .

# Run container
docker run -d -p 8888:8888 \
  -e GIT_URI=https://github.com/your-org/config-repo.git \
  -v vortex-repos:/var/lib/vortex/repos \
  vortex-config:latest
```

### Docker Compose

```bash
cd deployment
docker-compose up -d
```

### Kubernetes

See complete Kubernetes manifests in the [Deployment Guide](docs/wiki/Deployment.md) including:
- Deployment with replicas
- Service (ClusterIP)
- ConfigMap & Secrets
- PersistentVolumeClaim
- Ingress (optional)

**Key specs:**
- Memory: 64Mi (request) / 256Mi (limit)
- CPU: 100m (request) / 500m (limit)
- Health checks on `/health`
- Non-root user (UID 1000)

**[→ Complete Deployment Guide](docs/wiki/Deployment.md)**

## 🔧 Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `GIT_URI` | *required* | Git repository URL |
| `VORTEX_PORT` | `8888` | HTTP port |
| `VORTEX_CACHE_TTL_SECONDS` | `300` | Cache TTL |
| `GIT_REFRESH_INTERVAL_SECS` | `30` | Refresh interval |
| `RUST_LOG` | `info` | Log level |

**[→ Complete Configuration Guide](docs/wiki/Configuration.md)**

## ☁️ Spring Cloud Config Compatibility

**Drop-in replacement** - No client changes required:

```yaml
# Spring Boot application.yml
spring:
  application:
    name: myapp
  cloud:
    config:
      uri: http://vortex-config:8888
      profile: ${ENVIRONMENT}
      label: main
```

**File resolution order** (highest priority first):
1. `{app}-{profile}.yml`
2. `{app}.yml`
3. `application-{profile}.yml`
4. `application.yml`

## 🤝 Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Run tests: `cargo test --workspace`
4. Format code: `cargo fmt --all`
5. Lint: `cargo clippy --workspace -- -D warnings`
6. Open a Pull Request

**[→ Development Guide](docs/wiki/Development.md)**

## 📊 Project Metrics

| Metric | Value |
|--------|-------|
| **Completeness** | 50% (5/10 epics) |
| **Lines of Code** | ~7,100 Rust |
| **Tests** | 89 active tests |
| **Coverage** | >80% critical paths |
| **Build Time** | ~90s (release) |
| **Binary Size** | ~10MB |
| **Docker Image** | ~37MB |
| **Cold Start** | <500ms |
| **Memory (idle)** | ~20MB |

**[→ Detailed Status](STATUS.md)**

## 📄 License

**[Polyform Noncommercial License 1.0.0](LICENSE)**

- ✅ **Allowed:** Personal, research, education, non-profit use
- ❌ **Not allowed:** Commercial use without permission
- ⚠️ **No liability:** Use at your own risk

**Commercial licensing:** Contact [@cburgosro9303](https://github.com/cburgosro9303)

---

**Made with ⚡ by [Carlos Burgos](https://github.com/cburgosro9303)** | [Report Issue](https://github.com/cburgosro9303/vortex-config/issues) | [Request Feature](https://github.com/cburgosro9303/vortex-config/discussions)
