# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| Latest  | Yes                |
| Older   | No                 |

Only the latest release receives security updates.

## Reporting a Vulnerability

If you discover a security vulnerability in Vortex, please report it responsibly.

**Do NOT open a public issue.**

Instead, send an email to **cburgosro9303@github.com** with:

- A description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

You can expect an initial response within 72 hours. We will work with you to understand the issue and coordinate a fix before any public disclosure.

## Scope

This policy covers:

- The Vortex desktop binary
- All workspace crates (domain, application, infrastructure, ui, app)
- CI/CD workflows and release pipeline
- Import/export functionality (Postman, HAR, OpenAPI)

## Security Considerations

Vortex is a desktop API client that handles sensitive data such as API keys, tokens, and credentials. Key security measures in place:

- **`unsafe` code is forbidden**: `#![forbid(unsafe_code)]` is enforced across all crates via workspace lints
- **Secrets separation**: Sensitive values are stored separately from collection data via `FileSecretsRepository`
- **TLS by default**: All HTTP requests use `rustls` with no fallback to insecure connections
- **No telemetry**: Vortex collects zero usage data and makes no network calls except user-initiated API requests
- **Local-only storage**: All data is persisted to the local filesystem with no cloud sync
