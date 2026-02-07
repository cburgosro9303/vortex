# Contributing to Vortex

Thank you for your interest in contributing to Vortex!

## Getting Started

### Prerequisites

- Rust 1.93.0+
- System dependencies:
  - **Linux**: `sudo apt-get install -y libfontconfig1-dev libfreetype6-dev`
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`)
  - **Windows**: Visual Studio C++ Build Tools

### Building

```bash
# Build all crates
cargo build --workspace

# Run all tests
cargo test --workspace

# Check formatting
cargo fmt --all -- --check

# Run clippy lints
cargo clippy --workspace --all-targets -- -D warnings
```

## How to Contribute

### Reporting Issues

Use the [issue templates](https://github.com/cburgosro9303/vortex/issues/new/choose) to report bugs, request features, or suggest improvements.

### Submitting Changes

1. Fork the repository (if external contributor)
2. Create a feature branch from `main`: `git checkout -b feat/my-feature`
3. Make your changes following the conventions below
4. Ensure all tests pass
5. Commit using [Conventional Commits](https://www.conventionalcommits.org/)
6. Open a Pull Request against `main`

### Commit Convention

This project uses Conventional Commits for automatic semantic versioning:

- `feat:` — new feature (triggers minor version bump)
- `fix:` — bug fix (triggers patch version bump)
- `chore:` — maintenance tasks (no version bump)
- `docs:` — documentation changes
- `refactor:` — code refactoring
- `test:` — test additions or changes
- Breaking changes: add `!` after the type (e.g., `feat!:`) to trigger a major version bump

### Pull Request Requirements

- All CI checks must pass (fmt, clippy, build, test on all platforms)
- At least one approving review from a repository owner
- Follow the PR template provided

## Code Guidelines

### Rust

- Run `cargo fmt` before committing
- Run `cargo clippy` and fix all warnings
- `unsafe` code is **forbidden** (`#![forbid(unsafe_code)]` via workspace lints)
- Follow hexagonal architecture: domain has no I/O dependencies, infrastructure implements ports

### Architecture

Vortex uses a hexagonal (ports & adapters) architecture with 5 crates:

| Crate | Purpose |
|-------|---------|
| `domain` | Core business types, no I/O dependencies |
| `application` | Use cases, port definitions (traits) |
| `infrastructure` | Adapter implementations (HTTP, persistence, import/export) |
| `ui` | Slint UI components and view models |
| `app` | Binary entry point, dependency wiring |

**Dependency rule**: `app` -> `ui` -> `application` -> `domain` (and `infrastructure` implements `application` ports)

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
