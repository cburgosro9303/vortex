# Sprint 00 - Technical Foundations

**Objective:** Establish the architectural and technical foundation for the Vortex API Client.

**Milestone:** M0
**Duration:** 1-2 days
**Prerequisites:** Rust 1.93+, cargo, git

---

## Table of Contents

1. [Scope](#scope)
2. [Out of Scope](#out-of-scope)
3. [Final Directory Structure](#final-directory-structure)
4. [Implementation Order](#implementation-order)
5. [Task Details](#task-details)
6. [Acceptance Criteria](#acceptance-criteria)
7. [Verification Commands](#verification-commands)

---

## Scope

- Create multi-crate workspace with hexagonal architecture
- Define module boundaries and public contracts
- Integrate Slint with empty window application
- Configure local CI (rustfmt, clippy, tests)
- Establish error handling patterns

## Out of Scope

- HTTP request functionality
- Real persistence (SQLite/files)
- Import/export features
- Any business logic implementation

---

## Final Directory Structure

After completing Sprint 00, the project should have this exact structure:

```
vortex/
├── Cargo.toml                    # Workspace root
├── rust-toolchain.toml           # Rust version pinning
├── rustfmt.toml                  # Formatting configuration
├── clippy.toml                   # Clippy configuration
├── .gitignore
├── .github/
│   └── workflows/
│       └── ci.yml                # GitHub Actions CI
├── crates/
│   ├── domain/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs
│   │       ├── request/
│   │       │   ├── mod.rs
│   │       │   ├── method.rs
│   │       │   ├── header.rs
│   │       │   ├── body.rs
│   │       │   └── spec.rs
│   │       ├── response/
│   │       │   ├── mod.rs
│   │       │   └── spec.rs
│   │       ├── auth/
│   │       │   ├── mod.rs
│   │       │   └── types.rs
│   │       ├── environment/
│   │       │   ├── mod.rs
│   │       │   └── variable.rs
│   │       └── collection/
│   │           ├── mod.rs
│   │           └── item.rs
│   ├── application/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs
│   │       └── ports/
│   │           ├── mod.rs
│   │           ├── http_client.rs
│   │           ├── storage.rs
│   │           └── clock.rs
│   ├── infrastructure/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── adapters/
│   │           ├── mod.rs
│   │           └── system_clock.rs
│   ├── ui/
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app_window.rs
│   │       └── ui/
│   │           └── main_window.slint
│   └── app/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
└── tests/
    └── integration/
        └── workspace_compiles.rs
```

---

## Implementation Order

Tasks must be completed in this order due to dependencies:

```
[T01] Create workspace root
  │
  ├──► [T02] Create domain crate (no deps)
  │
  ├──► [T03] Create application crate (depends on domain)
  │
  ├──► [T04] Create infrastructure crate (depends on domain, application)
  │
  ├──► [T05] Create ui crate (depends on domain)
  │
  └──► [T06] Create app binary (depends on all crates)
        │
        └──► [T07] Configure CI and tooling
              │
              └──► [T08] Verification and cleanup
```

---

## Task Details

### T01: Create Workspace Root

**Files to create:**

#### `vortex/Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "crates/domain",
    "crates/application",
    "crates/infrastructure",
    "crates/ui",
    "crates/app",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.93.0"
authors = ["Vortex Team"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/your-org/vortex"

[workspace.dependencies]
# Internal crates
vortex-domain = { path = "crates/domain" }
vortex-application = { path = "crates/application" }
vortex-infrastructure = { path = "crates/infrastructure" }
vortex-ui = { path = "crates/ui" }

# Async runtime
tokio = { version = "1.43", features = ["rt-multi-thread", "macros", "time", "sync"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "2.0"

# UI Framework
slint = "1.9"

# HTTP Client (for future use)
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

# Utilities
uuid = { version = "1.11", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
url = { version = "2.5", features = ["serde"] }

# Testing
pretty_assertions = "1.4"

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
```

#### `vortex/rust-toolchain.toml`

```toml
[toolchain]
channel = "1.93.0"
components = ["rustfmt", "clippy"]
```

#### `vortex/rustfmt.toml`

```toml
edition = "2024"
max_width = 100
tab_spaces = 4
use_field_init_shorthand = true
use_try_shorthand = true
imports_granularity = "Module"
group_imports = "StdExternalCrate"
reorder_imports = true
reorder_modules = true
```

#### `vortex/clippy.toml`

```toml
cognitive-complexity-threshold = 15
too-many-arguments-threshold = 7
type-complexity-threshold = 250
```

#### `vortex/.gitignore`

```gitignore
# Build artifacts
/target/
**/target/

# IDE
.idea/
.vscode/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db

# Rust
**/*.rs.bk
Cargo.lock

# Environment
.env
.env.local
*.secret

# Logs
*.log
```

---

### T02: Create Domain Crate

The domain crate contains pure Rust types with no external I/O dependencies.

#### `vortex/crates/domain/Cargo.toml`

```toml
[package]
name = "vortex-domain"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "Domain types for Vortex API Client"

[dependencies]
serde = { workspace = true }
uuid = { workspace = true }
url = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
pretty_assertions = { workspace = true }
serde_json = { workspace = true }

[lints]
workspace = true
```

#### `vortex/crates/domain/src/lib.rs`

```rust
//! Vortex Domain - Core business types
//!
//! This crate defines the domain model for the Vortex API Client.
//! All types here are pure Rust with no I/O dependencies.

pub mod auth;
pub mod collection;
pub mod environment;
pub mod error;
pub mod request;
pub mod response;

pub use error::{DomainError, DomainResult};
```

#### `vortex/crates/domain/src/error.rs`

```rust
//! Domain error types

use thiserror::Error;

/// Domain-level errors that can occur during validation or processing.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// The provided URL is invalid or malformed.
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    /// A required header name is invalid.
    #[error("invalid header name: {0}")]
    InvalidHeaderName(String),

    /// A required header value is invalid.
    #[error("invalid header value: {0}")]
    InvalidHeaderValue(String),

    /// The HTTP method is not supported.
    #[error("unsupported HTTP method: {0}")]
    UnsupportedMethod(String),

    /// A variable reference is malformed.
    #[error("invalid variable reference: {0}")]
    InvalidVariableReference(String),

    /// The request body is invalid for the given content type.
    #[error("invalid body: {0}")]
    InvalidBody(String),

    /// A collection item has an invalid structure.
    #[error("invalid collection item: {0}")]
    InvalidCollectionItem(String),

    /// An identifier is invalid or empty.
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),
}

/// Result type alias for domain operations.
pub type DomainResult<T> = Result<T, DomainError>;
```

#### `vortex/crates/domain/src/request/mod.rs`

```rust
//! HTTP Request domain types

mod body;
mod header;
mod method;
mod spec;

pub use body::{RequestBody, RequestBodyKind};
pub use header::{Header, Headers};
pub use method::HttpMethod;
pub use spec::RequestSpec;
```

#### `vortex/crates/domain/src/request/method.rs`

```rust
//! HTTP Method enumeration

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::error::{DomainError, DomainResult};

/// Supported HTTP methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// HTTP GET method
    #[default]
    Get,
    /// HTTP POST method
    Post,
    /// HTTP PUT method
    Put,
    /// HTTP PATCH method
    Patch,
    /// HTTP DELETE method
    Delete,
    /// HTTP HEAD method
    Head,
    /// HTTP OPTIONS method
    Options,
}

impl HttpMethod {
    /// Returns all available HTTP methods.
    #[must_use]
    pub const fn all() -> &'static [HttpMethod] {
        &[
            HttpMethod::Get,
            HttpMethod::Post,
            HttpMethod::Put,
            HttpMethod::Patch,
            HttpMethod::Delete,
            HttpMethod::Head,
            HttpMethod::Options,
        ]
    }

    /// Returns whether this method typically has a request body.
    #[must_use]
    pub const fn has_body(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch)
    }

    /// Returns the method as a static string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for HttpMethod {
    type Err = DomainError;

    fn from_str(s: &str) -> DomainResult<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            "DELETE" => Ok(Self::Delete),
            "HEAD" => Ok(Self::Head),
            "OPTIONS" => Ok(Self::Options),
            other => Err(DomainError::UnsupportedMethod(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_method_from_str() {
        assert_eq!("get".parse::<HttpMethod>().unwrap(), HttpMethod::Get);
        assert_eq!("POST".parse::<HttpMethod>().unwrap(), HttpMethod::Post);
        assert_eq!("Put".parse::<HttpMethod>().unwrap(), HttpMethod::Put);
    }

    #[test]
    fn test_method_display() {
        assert_eq!(HttpMethod::Get.to_string(), "GET");
        assert_eq!(HttpMethod::Post.to_string(), "POST");
    }

    #[test]
    fn test_invalid_method() {
        let result = "INVALID".parse::<HttpMethod>();
        assert!(result.is_err());
    }

    #[test]
    fn test_has_body() {
        assert!(!HttpMethod::Get.has_body());
        assert!(HttpMethod::Post.has_body());
        assert!(HttpMethod::Put.has_body());
        assert!(HttpMethod::Patch.has_body());
        assert!(!HttpMethod::Delete.has_body());
    }
}
```

#### `vortex/crates/domain/src/request/header.rs`

```rust
//! HTTP Header types

use serde::{Deserialize, Serialize};

/// A single HTTP header with name and value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// The header name (e.g., "Content-Type")
    pub name: String,
    /// The header value (e.g., "application/json")
    pub value: String,
    /// Whether this header is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Header {
    /// Creates a new enabled header.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            enabled: true,
        }
    }

    /// Creates a new disabled header.
    #[must_use]
    pub fn disabled(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            enabled: false,
        }
    }
}

/// A collection of HTTP headers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Headers {
    items: Vec<Header>,
}

impl Headers {
    /// Creates an empty header collection.
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Adds a header to the collection.
    pub fn add(&mut self, header: Header) {
        self.items.push(header);
    }

    /// Returns an iterator over enabled headers.
    pub fn enabled(&self) -> impl Iterator<Item = &Header> {
        self.items.iter().filter(|h| h.enabled)
    }

    /// Returns all headers (enabled and disabled).
    pub fn all(&self) -> &[Header] {
        &self.items
    }

    /// Returns the number of headers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if there are no headers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl FromIterator<Header> for Headers {
    fn from_iter<T: IntoIterator<Item = Header>>(iter: T) -> Self {
        Self {
            items: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_creation() {
        let header = Header::new("Content-Type", "application/json");
        assert_eq!(header.name, "Content-Type");
        assert_eq!(header.value, "application/json");
        assert!(header.enabled);
    }

    #[test]
    fn test_disabled_header() {
        let header = Header::disabled("X-Debug", "true");
        assert!(!header.enabled);
    }

    #[test]
    fn test_headers_filter_enabled() {
        let mut headers = Headers::new();
        headers.add(Header::new("Accept", "application/json"));
        headers.add(Header::disabled("X-Debug", "true"));
        headers.add(Header::new("User-Agent", "Vortex"));

        let enabled: Vec<_> = headers.enabled().collect();
        assert_eq!(enabled.len(), 2);
    }
}
```

#### `vortex/crates/domain/src/request/body.rs`

```rust
//! HTTP Request body types

use serde::{Deserialize, Serialize};

/// The kind of request body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequestBodyKind {
    /// No body
    #[default]
    None,
    /// Raw text/JSON body
    Raw {
        /// The content type (e.g., "application/json", "text/plain")
        content_type: String,
    },
    /// Form URL encoded body
    FormUrlEncoded,
    /// Multipart form data
    FormData,
}

/// HTTP request body with content and type information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RequestBody {
    /// The kind of body
    pub kind: RequestBodyKind,
    /// The body content as a string
    #[serde(default)]
    pub content: String,
}

impl RequestBody {
    /// Creates an empty body.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            kind: RequestBodyKind::None,
            content: String::new(),
        }
    }

    /// Creates a JSON body.
    #[must_use]
    pub fn json(content: impl Into<String>) -> Self {
        Self {
            kind: RequestBodyKind::Raw {
                content_type: "application/json".to_string(),
            },
            content: content.into(),
        }
    }

    /// Creates a plain text body.
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            kind: RequestBodyKind::Raw {
                content_type: "text/plain".to_string(),
            },
            content: content.into(),
        }
    }

    /// Returns whether the body is empty or none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self.kind, RequestBodyKind::None) || self.content.is_empty()
    }

    /// Returns the content type if applicable.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        match &self.kind {
            RequestBodyKind::None => None,
            RequestBodyKind::Raw { content_type } => Some(content_type),
            RequestBodyKind::FormUrlEncoded => Some("application/x-www-form-urlencoded"),
            RequestBodyKind::FormData => Some("multipart/form-data"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_body() {
        let body = RequestBody::json(r#"{"key": "value"}"#);
        assert_eq!(body.content_type(), Some("application/json"));
        assert!(!body.is_empty());
    }

    #[test]
    fn test_empty_body() {
        let body = RequestBody::none();
        assert!(body.is_empty());
        assert_eq!(body.content_type(), None);
    }
}
```

#### `vortex/crates/domain/src/request/spec.rs`

```rust
//! Request specification type

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::{Headers, HttpMethod, RequestBody};
use crate::auth::AuthConfig;

/// Complete specification for an HTTP request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestSpec {
    /// Unique identifier for this request
    pub id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Optional description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// HTTP method
    pub method: HttpMethod,
    /// Target URL (may contain variable placeholders)
    pub url: String,
    /// HTTP headers
    #[serde(default)]
    pub headers: Headers,
    /// Request body
    #[serde(default)]
    pub body: RequestBody,
    /// Authentication configuration
    #[serde(default)]
    pub auth: AuthConfig,
}

impl RequestSpec {
    /// Creates a new request specification with default values.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            method: HttpMethod::default(),
            url: String::new(),
            headers: Headers::new(),
            body: RequestBody::none(),
            auth: AuthConfig::default(),
        }
    }

    /// Creates a GET request with the given URL.
    #[must_use]
    pub fn get(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            method: HttpMethod::Get,
            url: url.into(),
            headers: Headers::new(),
            body: RequestBody::none(),
            auth: AuthConfig::default(),
        }
    }

    /// Validates the URL and returns parsed version if valid.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is malformed.
    pub fn parse_url(&self) -> Result<Url, url::ParseError> {
        Url::parse(&self.url)
    }

    /// Returns true if the URL contains variable placeholders.
    #[must_use]
    pub fn has_variables(&self) -> bool {
        self.url.contains("{{") && self.url.contains("}}")
    }
}

impl Default for RequestSpec {
    fn default() -> Self {
        Self::new("New Request")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_request() {
        let req = RequestSpec::new("Test Request");
        assert_eq!(req.name, "Test Request");
        assert_eq!(req.method, HttpMethod::Get);
    }

    #[test]
    fn test_get_request() {
        let req = RequestSpec::get("Users", "https://api.example.com/users");
        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.url, "https://api.example.com/users");
    }

    #[test]
    fn test_has_variables() {
        let mut req = RequestSpec::new("Test");
        req.url = "https://{{host}}/api/{{version}}/users".to_string();
        assert!(req.has_variables());

        req.url = "https://api.example.com/users".to_string();
        assert!(!req.has_variables());
    }
}
```

#### `vortex/crates/domain/src/response/mod.rs`

```rust
//! HTTP Response domain types

mod spec;

pub use spec::ResponseSpec;
```

#### `vortex/crates/domain/src/response/spec.rs`

```rust
//! Response specification type

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::request::Headers;

/// HTTP response specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseSpec {
    /// HTTP status code
    pub status: u16,
    /// Status text (e.g., "OK", "Not Found")
    pub status_text: String,
    /// Response headers
    pub headers: Headers,
    /// Response body as string
    pub body: String,
    /// Response time
    #[serde(with = "duration_millis")]
    pub duration: Duration,
    /// Response size in bytes
    pub size: usize,
}

impl ResponseSpec {
    /// Returns true if the status code indicates success (2xx).
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// Returns true if the status code indicates a client error (4xx).
    #[must_use]
    pub const fn is_client_error(&self) -> bool {
        self.status >= 400 && self.status < 500
    }

    /// Returns true if the status code indicates a server error (5xx).
    #[must_use]
    pub const fn is_server_error(&self) -> bool {
        self.status >= 500 && self.status < 600
    }
}

impl Default for ResponseSpec {
    fn default() -> Self {
        Self {
            status: 0,
            status_text: String::new(),
            headers: Headers::new(),
            body: String::new(),
            duration: Duration::ZERO,
            size: 0,
        }
    }
}

mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_checks() {
        let mut response = ResponseSpec::default();

        response.status = 200;
        assert!(response.is_success());
        assert!(!response.is_client_error());
        assert!(!response.is_server_error());

        response.status = 404;
        assert!(!response.is_success());
        assert!(response.is_client_error());

        response.status = 500;
        assert!(response.is_server_error());
    }
}
```

#### `vortex/crates/domain/src/auth/mod.rs`

```rust
//! Authentication domain types

mod types;

pub use types::AuthConfig;
```

#### `vortex/crates/domain/src/auth/types.rs`

```rust
//! Authentication configuration types

use serde::{Deserialize, Serialize};

/// Authentication configuration for a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// No authentication
    #[default]
    None,
    /// API Key authentication
    ApiKey {
        /// The API key value
        key: String,
        /// Header or query parameter name
        name: String,
        /// Where to add the key
        location: ApiKeyLocation,
    },
    /// Bearer token authentication
    Bearer {
        /// The bearer token
        token: String,
    },
    /// Basic authentication
    Basic {
        /// Username
        username: String,
        /// Password
        password: String,
    },
}

/// Location for API key authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyLocation {
    /// Add to request headers
    #[default]
    Header,
    /// Add to query parameters
    Query,
}

impl AuthConfig {
    /// Returns true if authentication is configured.
    #[must_use]
    pub const fn is_configured(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Creates a bearer token authentication.
    #[must_use]
    pub fn bearer(token: impl Into<String>) -> Self {
        Self::Bearer {
            token: token.into(),
        }
    }

    /// Creates a basic authentication.
    #[must_use]
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Creates an API key authentication in header.
    #[must_use]
    pub fn api_key_header(name: impl Into<String>, key: impl Into<String>) -> Self {
        Self::ApiKey {
            key: key.into(),
            name: name.into(),
            location: ApiKeyLocation::Header,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_none() {
        let auth = AuthConfig::None;
        assert!(!auth.is_configured());
    }

    #[test]
    fn test_bearer_auth() {
        let auth = AuthConfig::bearer("my-token");
        assert!(auth.is_configured());
        if let AuthConfig::Bearer { token } = auth {
            assert_eq!(token, "my-token");
        } else {
            panic!("Expected Bearer auth");
        }
    }
}
```

#### `vortex/crates/domain/src/environment/mod.rs`

```rust
//! Environment and variable domain types

mod variable;

pub use variable::{Environment, Variable};
```

#### `vortex/crates/domain/src/environment/variable.rs`

```rust
//! Environment variable types

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single environment variable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variable {
    /// Variable name (used in placeholders like {{name}})
    pub name: String,
    /// Variable value
    pub value: String,
    /// Whether this is a secret (should be masked in UI)
    #[serde(default)]
    pub is_secret: bool,
    /// Whether this variable is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Variable {
    /// Creates a new enabled variable.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            is_secret: false,
            enabled: true,
        }
    }

    /// Creates a new secret variable.
    #[must_use]
    pub fn secret(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            is_secret: true,
            enabled: true,
        }
    }
}

/// An environment containing a set of variables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    /// Unique identifier
    pub id: Uuid,
    /// Environment name (e.g., "Development", "Production")
    pub name: String,
    /// Variables in this environment
    #[serde(default)]
    pub variables: Vec<Variable>,
}

impl Environment {
    /// Creates a new empty environment.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            variables: Vec::new(),
        }
    }

    /// Adds a variable to the environment.
    pub fn add_variable(&mut self, variable: Variable) {
        self.variables.push(variable);
    }

    /// Gets a variable by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Variable> {
        self.variables
            .iter()
            .find(|v| v.name == name && v.enabled)
    }

    /// Resolves a placeholder value.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.get(name).map(|v| v.value.as_str())
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new("Default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_resolve() {
        let mut env = Environment::new("Test");
        env.add_variable(Variable::new("host", "api.example.com"));
        env.add_variable(Variable::new("port", "8080"));

        assert_eq!(env.resolve("host"), Some("api.example.com"));
        assert_eq!(env.resolve("port"), Some("8080"));
        assert_eq!(env.resolve("unknown"), None);
    }

    #[test]
    fn test_disabled_variable() {
        let mut env = Environment::new("Test");
        let mut var = Variable::new("disabled", "value");
        var.enabled = false;
        env.add_variable(var);

        assert_eq!(env.resolve("disabled"), None);
    }
}
```

#### `vortex/crates/domain/src/collection/mod.rs`

```rust
//! Collection domain types

mod item;

pub use item::{Collection, CollectionItem, Folder};
```

#### `vortex/crates/domain/src/collection/item.rs`

```rust
//! Collection item types

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::request::RequestSpec;

/// A folder containing requests and other folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// Unique identifier
    pub id: Uuid,
    /// Folder name
    pub name: String,
    /// Items in this folder
    #[serde(default)]
    pub items: Vec<CollectionItem>,
}

impl Folder {
    /// Creates a new empty folder.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            items: Vec::new(),
        }
    }
}

/// An item in a collection (either a folder or a request).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CollectionItem {
    /// A folder containing other items
    Folder(Folder),
    /// A request specification
    Request(RequestSpec),
}

impl CollectionItem {
    /// Returns the ID of this item.
    #[must_use]
    pub fn id(&self) -> Uuid {
        match self {
            Self::Folder(f) => f.id,
            Self::Request(r) => r.id,
        }
    }

    /// Returns the name of this item.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Folder(f) => &f.name,
            Self::Request(r) => &r.name,
        }
    }
}

/// A collection of requests organized in folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    /// Schema version for migration support
    pub schema: u32,
    /// Unique identifier
    pub id: Uuid,
    /// Collection name
    pub name: String,
    /// Optional description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Items in this collection
    #[serde(default)]
    pub items: Vec<CollectionItem>,
}

impl Collection {
    /// Current schema version.
    pub const SCHEMA_VERSION: u32 = 1;

    /// Creates a new empty collection.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema: Self::SCHEMA_VERSION,
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            items: Vec::new(),
        }
    }

    /// Adds an item to the collection root.
    pub fn add_item(&mut self, item: CollectionItem) {
        self.items.push(item);
    }

    /// Returns the total number of requests in the collection (recursive).
    #[must_use]
    pub fn request_count(&self) -> usize {
        fn count_in_items(items: &[CollectionItem]) -> usize {
            items.iter().fold(0, |acc, item| {
                acc + match item {
                    CollectionItem::Request(_) => 1,
                    CollectionItem::Folder(f) => count_in_items(&f.items),
                }
            })
        }
        count_in_items(&self.items)
    }
}

impl Default for Collection {
    fn default() -> Self {
        Self::new("New Collection")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_creation() {
        let collection = Collection::new("My API");
        assert_eq!(collection.name, "My API");
        assert_eq!(collection.schema, Collection::SCHEMA_VERSION);
        assert!(collection.items.is_empty());
    }

    #[test]
    fn test_request_count() {
        let mut collection = Collection::new("Test");

        // Add a request at root
        collection.add_item(CollectionItem::Request(RequestSpec::new("Request 1")));

        // Add a folder with requests
        let mut folder = Folder::new("Users");
        folder.items.push(CollectionItem::Request(RequestSpec::new("Get Users")));
        folder.items.push(CollectionItem::Request(RequestSpec::new("Create User")));
        collection.add_item(CollectionItem::Folder(folder));

        assert_eq!(collection.request_count(), 3);
    }
}
```

---

### T03: Create Application Crate

The application crate defines ports (interfaces) and orchestrates use cases.

#### `vortex/crates/application/Cargo.toml`

```toml
[package]
name = "vortex-application"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "Application layer for Vortex API Client"

[dependencies]
vortex-domain = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
chrono = { workspace = true }

[dev-dependencies]
pretty_assertions = { workspace = true }

[lints]
workspace = true
```

#### `vortex/crates/application/src/lib.rs`

```rust
//! Vortex Application - Use cases and ports
//!
//! This crate defines the application layer with:
//! - Port traits (interfaces for external dependencies)
//! - Use case orchestration
//! - Application-level error handling

pub mod error;
pub mod ports;

pub use error::{ApplicationError, ApplicationResult};
```

#### `vortex/crates/application/src/error.rs`

```rust
//! Application error types

use thiserror::Error;
use vortex_domain::DomainError;

/// Application-level errors.
#[derive(Debug, Error)]
pub enum ApplicationError {
    /// A domain validation error occurred.
    #[error("domain error: {0}")]
    Domain(#[from] DomainError),

    /// An HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(String),

    /// A storage operation failed.
    #[error("storage error: {0}")]
    Storage(String),

    /// The requested resource was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// An internal error occurred.
    #[error("internal error: {0}")]
    Internal(String),

    /// The operation timed out.
    #[error("operation timed out")]
    Timeout,

    /// The operation was cancelled.
    #[error("operation cancelled")]
    Cancelled,
}

/// Result type alias for application operations.
pub type ApplicationResult<T> = Result<T, ApplicationError>;
```

#### `vortex/crates/application/src/ports/mod.rs`

```rust
//! Port definitions (interfaces)
//!
//! Ports define the boundaries between the application core and external systems.
//! Each port is a trait that can be implemented by adapters in the infrastructure layer.

mod clock;
mod http_client;
mod storage;

pub use clock::Clock;
pub use http_client::HttpClient;
pub use storage::{CollectionStorage, EnvironmentStorage};
```

#### `vortex/crates/application/src/ports/http_client.rs`

```rust
//! HTTP Client port

use std::future::Future;

use vortex_domain::{request::RequestSpec, response::ResponseSpec};

use crate::ApplicationResult;

/// Port for executing HTTP requests.
///
/// This trait abstracts the HTTP client implementation, allowing
/// the application layer to be independent of specific HTTP libraries.
pub trait HttpClient: Send + Sync {
    /// Executes an HTTP request and returns the response.
    ///
    /// # Arguments
    ///
    /// * `request` - The request specification to execute
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails due to network issues,
    /// timeout, or other HTTP-related problems.
    fn execute(
        &self,
        request: &RequestSpec,
    ) -> impl Future<Output = ApplicationResult<ResponseSpec>> + Send;

    /// Cancels any pending request with the given ID.
    ///
    /// This is a best-effort operation; the request may still complete
    /// if it was already in flight.
    fn cancel(&self, request_id: uuid::Uuid) -> impl Future<Output = ()> + Send;
}
```

#### `vortex/crates/application/src/ports/storage.rs`

```rust
//! Storage ports

use std::future::Future;
use std::path::Path;

use vortex_domain::{collection::Collection, environment::Environment};

use crate::ApplicationResult;

/// Port for persisting and loading collections.
pub trait CollectionStorage: Send + Sync {
    /// Saves a collection to the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the collection cannot be serialized or written.
    fn save(
        &self,
        collection: &Collection,
        path: &Path,
    ) -> impl Future<Output = ApplicationResult<()>> + Send;

    /// Loads a collection from the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    fn load(&self, path: &Path) -> impl Future<Output = ApplicationResult<Collection>> + Send;

    /// Lists all collections in the specified directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    fn list(&self, directory: &Path) -> impl Future<Output = ApplicationResult<Vec<Collection>>> + Send;
}

/// Port for persisting and loading environments.
pub trait EnvironmentStorage: Send + Sync {
    /// Saves an environment to the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the environment cannot be serialized or written.
    fn save(
        &self,
        environment: &Environment,
        path: &Path,
    ) -> impl Future<Output = ApplicationResult<()>> + Send;

    /// Loads an environment from the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    fn load(&self, path: &Path) -> impl Future<Output = ApplicationResult<Environment>> + Send;
}
```

#### `vortex/crates/application/src/ports/clock.rs`

```rust
//! Clock port for time-related operations

use chrono::{DateTime, Utc};

/// Port for getting the current time.
///
/// This abstraction allows testing time-dependent code by providing
/// a mock implementation.
pub trait Clock: Send + Sync {
    /// Returns the current UTC timestamp.
    fn now(&self) -> DateTime<Utc>;
}
```

---

### T04: Create Infrastructure Crate

The infrastructure crate provides real implementations of the ports.

#### `vortex/crates/infrastructure/Cargo.toml`

```toml
[package]
name = "vortex-infrastructure"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "Infrastructure adapters for Vortex API Client"

[dependencies]
vortex-domain = { workspace = true }
vortex-application = { workspace = true }
chrono = { workspace = true }

[dev-dependencies]
pretty_assertions = { workspace = true }

[lints]
workspace = true
```

#### `vortex/crates/infrastructure/src/lib.rs`

```rust
//! Vortex Infrastructure - Adapters and implementations
//!
//! This crate provides concrete implementations of the ports
//! defined in the application layer.

pub mod adapters;
```

#### `vortex/crates/infrastructure/src/adapters/mod.rs`

```rust
//! Infrastructure adapters

mod system_clock;

pub use system_clock::SystemClock;
```

#### `vortex/crates/infrastructure/src/adapters/system_clock.rs`

```rust
//! System clock adapter

use chrono::{DateTime, Utc};
use vortex_application::ports::Clock;

/// System clock implementation using the system time.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl SystemClock {
    /// Creates a new system clock.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_clock() {
        let clock = SystemClock::new();
        let now = clock.now();
        // Just verify it returns a reasonable timestamp
        assert!(now.timestamp() > 0);
    }
}
```

---

### T05: Create UI Crate

The UI crate contains Slint components and view models.

#### `vortex/crates/ui/Cargo.toml`

```toml
[package]
name = "vortex-ui"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "UI layer for Vortex API Client"
build = "build.rs"

[dependencies]
vortex-domain = { workspace = true }
slint = { workspace = true }

[build-dependencies]
slint-build = "1.9"

[dev-dependencies]
pretty_assertions = { workspace = true }

[lints]
workspace = true
```

#### `vortex/crates/ui/build.rs`

```rust
fn main() {
    slint_build::compile("src/ui/main_window.slint").expect("Slint compilation failed");
}
```

#### `vortex/crates/ui/src/lib.rs`

```rust
//! Vortex UI - User interface layer
//!
//! This crate provides the Slint-based user interface for the Vortex API Client.

mod app_window;

pub use app_window::AppWindow;

// Include the generated Slint code
slint::include_modules!();
```

#### `vortex/crates/ui/src/app_window.rs`

```rust
//! Application window management

use slint::ComponentHandle;

use crate::MainWindow;

/// Application window wrapper with business logic bindings.
pub struct AppWindow {
    window: MainWindow,
}

impl AppWindow {
    /// Creates a new application window.
    ///
    /// # Errors
    ///
    /// Returns an error if the window cannot be created.
    pub fn new() -> Result<Self, slint::PlatformError> {
        let window = MainWindow::new()?;
        Ok(Self { window })
    }

    /// Runs the application event loop.
    ///
    /// This method blocks until the window is closed.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop fails.
    pub fn run(&self) -> Result<(), slint::PlatformError> {
        self.window.run()
    }

    /// Returns a reference to the underlying Slint window.
    #[must_use]
    pub fn window(&self) -> &MainWindow {
        &self.window
    }
}

impl Default for AppWindow {
    fn default() -> Self {
        Self::new().expect("Failed to create application window")
    }
}
```

#### `vortex/crates/ui/src/ui/main_window.slint`

```slint
// Vortex API Client - Main Window
// Sprint 00: Empty window with basic structure

import { VerticalBox, HorizontalBox, Button, LineEdit } from "std-widgets.slint";

// Application color palette
export global Theme {
    // Background colors
    out property <color> background-primary: #1e1e2e;
    out property <color> background-secondary: #313244;
    out property <color> background-tertiary: #45475a;

    // Text colors
    out property <color> text-primary: #cdd6f4;
    out property <color> text-secondary: #a6adc8;
    out property <color> text-muted: #6c7086;

    // Accent colors
    out property <color> accent-primary: #89b4fa;
    out property <color> accent-success: #a6e3a1;
    out property <color> accent-warning: #f9e2af;
    out property <color> accent-error: #f38ba8;

    // Spacing
    out property <length> spacing-xs: 4px;
    out property <length> spacing-sm: 8px;
    out property <length> spacing-md: 16px;
    out property <length> spacing-lg: 24px;
    out property <length> spacing-xl: 32px;
}

// Main application window
export component MainWindow inherits Window {
    title: "Vortex API Client";
    min-width: 1024px;
    min-height: 768px;
    preferred-width: 1280px;
    preferred-height: 800px;
    background: Theme.background-primary;

    // Placeholder content for Sprint 00
    VerticalBox {
        alignment: center;
        spacing: Theme.spacing-md;

        Text {
            text: "Vortex API Client";
            font-size: 32px;
            font-weight: 700;
            color: Theme.text-primary;
            horizontal-alignment: center;
        }

        Text {
            text: "Sprint 00 - Technical Foundations";
            font-size: 16px;
            color: Theme.text-secondary;
            horizontal-alignment: center;
        }

        Rectangle {
            height: Theme.spacing-xl;
        }

        Text {
            text: "Ready for development";
            font-size: 14px;
            color: Theme.text-muted;
            horizontal-alignment: center;
        }
    }
}
```

---

### T06: Create App Binary

The app crate is the entry point that wires everything together.

#### `vortex/crates/app/Cargo.toml`

```toml
[package]
name = "vortex"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "Vortex API Client - Desktop application"

[[bin]]
name = "vortex"
path = "src/main.rs"

[dependencies]
vortex-domain = { workspace = true }
vortex-application = { workspace = true }
vortex-infrastructure = { workspace = true }
vortex-ui = { workspace = true }
tokio = { workspace = true }

[lints]
workspace = true
```

#### `vortex/crates/app/src/main.rs`

```rust
//! Vortex API Client - Main Entry Point
//!
//! This is the desktop application entry point that initializes
//! all components and starts the UI event loop.

use vortex_ui::AppWindow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the application window
    let app = AppWindow::new()?;

    // Run the event loop (blocks until window closes)
    app.run()?;

    Ok(())
}
```

---

### T07: Configure CI and Tooling

#### `vortex/.github/workflows/ci.yml`

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  check:
    name: Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-action@stable
        with:
          toolchain: 1.93.0
          components: rustfmt, clippy

      - name: Install system dependencies (Linux)
        run: |
          sudo apt-get update
          sudo apt-get install -y libfontconfig1-dev libfreetype6-dev

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Run clippy
        run: cargo clippy --workspace --all-targets -- -D warnings

      - name: Build
        run: cargo build --workspace

      - name: Run tests
        run: cargo test --workspace

  build-macos:
    name: Build (macOS)
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-action@stable
        with:
          toolchain: 1.93.0

      - name: Build
        run: cargo build --workspace

  build-windows:
    name: Build (Windows)
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-action@stable
        with:
          toolchain: 1.93.0

      - name: Build
        run: cargo build --workspace
```

---

### T08: Verification and Cleanup

After implementing all tasks, run the following verification steps:

#### Create Integration Test

#### `vortex/tests/integration/workspace_compiles.rs`

```rust
//! Integration test to verify the workspace compiles correctly.

#[test]
fn domain_crate_compiles() {
    // Verify domain types are accessible
    let _method = vortex_domain::request::HttpMethod::Get;
    let _request = vortex_domain::request::RequestSpec::new("Test");
    let _collection = vortex_domain::collection::Collection::new("Test");
    let _env = vortex_domain::environment::Environment::new("Test");
}

#[test]
fn application_crate_compiles() {
    // Verify application types are accessible
    let _error = vortex_application::ApplicationError::Timeout;
}

#[test]
fn infrastructure_crate_compiles() {
    // Verify infrastructure adapters are accessible
    use vortex_application::ports::Clock;
    let clock = vortex_infrastructure::adapters::SystemClock::new();
    let _now = clock.now();
}
```

#### `vortex/tests/integration/mod.rs`

```rust
mod workspace_compiles;
```

---

## Acceptance Criteria

All the following must pass before Sprint 00 is complete:

| Criterion | Verification Command |
|-----------|---------------------|
| Project compiles without errors | `cargo build --workspace` |
| No clippy warnings | `cargo clippy --workspace --all-targets -- -D warnings` |
| Code is formatted | `cargo fmt --all -- --check` |
| All tests pass | `cargo test --workspace` |
| Application runs and shows window | `cargo run -p vortex` |
| Domain crate has no I/O dependencies | Check `crates/domain/Cargo.toml` |
| Crates have correct dependency direction | Domain <- Application <- Infrastructure |

---

## Verification Commands

Run these commands from the workspace root (`vortex/`) to verify the sprint is complete:

```bash
# 1. Check formatting
cargo fmt --all -- --check

# 2. Run clippy
cargo clippy --workspace --all-targets -- -D warnings

# 3. Build all crates
cargo build --workspace

# 4. Run all tests
cargo test --workspace

# 5. Run the application (should show empty window)
cargo run -p vortex

# 6. Verify dependency graph (optional, requires cargo-depgraph)
# cargo depgraph | dot -Tpng -o deps.png
```

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Circular dependencies between crates | Strict layering: domain has no internal deps, application depends only on domain |
| Slint build failures | Pin slint version, verify build.rs path |
| Platform-specific issues | CI runs on Linux, macOS, and Windows |
| Early coupling between UI and logic | UI crate only imports domain types, never application or infrastructure |

---

## Notes for AI Agents

1. **Create directories before files**: Use `mkdir -p` to create the directory structure first.

2. **File creation order**:
   - Root `Cargo.toml` first
   - Then crates in dependency order: domain -> application -> infrastructure -> ui -> app
   - Finally, CI configuration and tests

3. **Validation after each crate**: After creating each crate, run `cargo check -p <crate-name>` to verify it compiles.

4. **Slint requires build step**: The UI crate needs `slint-build` to compile `.slint` files.

5. **All paths are relative to workspace root**: The workspace root is `vortex/`.

6. **Do not skip any files**: Every file listed in the directory structure must be created.

---

## Milestone: M0 Completion

When all acceptance criteria pass, Milestone M0 (Architecture base ready) is complete, and the project is ready for Sprint 01 (MVP Execution UI).
