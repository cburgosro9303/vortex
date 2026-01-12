# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.1](https://github.com/cburgosro9303/vortex-config/compare/vortex-config-v1.0.0...vortex-config-v1.0.1) (2026-01-12)


### Bug Fixes

* add [package] section to root Cargo.toml ([5d7907d](https://github.com/cburgosro9303/vortex-config/commit/5d7907dffc40c739dbdae1c9abf3c8746c728ba4))
* configure release-please for rust workspace ([582f67e](https://github.com/cburgosro9303/vortex-config/commit/582f67e65c2db0194c59dda74c1547693297f2ae))
* use explicit versions in all crates for release-please ([c343b64](https://github.com/cburgosro9303/vortex-config/commit/c343b646a17b9e03ead5ee1be69ef44de24c955d))
* use rust release-type with bootstrap-sha ([2af4860](https://github.com/cburgosro9303/vortex-config/commit/2af4860abc597398a1681041a0a0b6ff812b78be))

## [1.0.0] - 2026-01-12

### Added

- **Spring Cloud Config API compatibility** - Drop-in replacement for Spring Cloud Config Server
- **Git backend with auto-refresh** - Clone, fetch, auto-refresh, branches/tags support
- **Moka async cache** - TTL-based cache with invalidation and metrics
- **Multi-format support** - JSON, YAML, Java Properties serialization
- **HTTP server with Axum 0.8** - High-performance async server
- **Prometheus metrics** - Built-in observability with metrics collection
- **CI/CD pipeline** - GitHub Actions with coverage reporting via Codecov
- **Docker deployment** - Production-ready Docker image (~37MB)
- **Comprehensive documentation** - Wiki, API reference, deployment guides

### Changed

- Migrated UUID from v4 to v7 for better time-based ordering
- Optimized release profile for smaller binary size

### Technical Details

- Rust 2024 edition with nightly toolchain
- Complete implementation of first 5 epics
- 89 active tests with >80% coverage on critical paths
- Cold start < 500ms, latency p99 < 10ms
- Memory footprint ~20MB idle

[1.0.0]: https://github.com/cburgosro9303/vortex-config/releases/tag/v1.0.0
