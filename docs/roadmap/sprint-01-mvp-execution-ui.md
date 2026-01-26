# Sprint 01 — MVP Request Execution + Base UI

**Objective:** Enable creating and executing HTTP requests with response visualization.

**Duration:** 1 week
**Milestone:** M1 (Alpha)

---

## Table of Contents

1. [Scope and Boundaries](#scope-and-boundaries)
2. [Project Structure](#project-structure)
3. [Domain Types (Rust)](#domain-types-rust)
4. [Application Layer](#application-layer)
5. [Infrastructure Layer](#infrastructure-layer)
6. [UI Layer (Slint)](#ui-layer-slint)
7. [Integration: Connecting UI to Domain](#integration-connecting-ui-to-domain)
8. [Implementation Order](#implementation-order)
9. [Testing Strategy](#testing-strategy)
10. [Acceptance Criteria](#acceptance-criteria)

---

## Scope and Boundaries

### In Scope
- Domain models: `RequestSpec`, `ResponseSpec`, `HttpMethod`, `RequestBody`, `RequestState`
- Use case: `ExecuteRequest` with async HTTP execution
- Base UI: 3-column layout with URL bar, method selector, send button, response panel
- Error handling with user-friendly messages
- Loading states and cancellation support

### Out of Scope
- File persistence (disk storage)
- Environments and variables (`{{variable}}` interpolation)
- Authentication (Bearer, OAuth, API Key)
- Collections and folders
- Request history
- Tabs for multiple requests

---

## Project Structure

```
vortex/
├── Cargo.toml                    # Workspace manifest
├── crates/
│   ├── vortex-domain/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── request.rs        # RequestSpec, HttpMethod, RequestBody
│   │       ├── response.rs       # ResponseSpec
│   │       └── state.rs          # RequestState enum
│   ├── vortex-application/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ports.rs          # HttpClient trait (port)
│   │       └── execute_request.rs # Use case implementation
│   ├── vortex-infrastructure/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── http_client.rs    # reqwest adapter
│   └── vortex-ui/
│       ├── Cargo.toml
│       ├── src/
│       │   ├── main.rs           # Entry point
│       │   └── bridge.rs         # UI-to-domain bridge
│       └── ui/
│           ├── main.slint        # Main window
│           ├── components/
│           │   ├── url_bar.slint
│           │   ├── method_selector.slint
│           │   └── response_panel.slint
│           └── theme.slint       # Colors and typography
└── rust-toolchain.toml
```

### Workspace Cargo.toml

```toml
# /vortex/Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/vortex-domain",
    "crates/vortex-application",
    "crates/vortex-infrastructure",
    "crates/vortex-ui",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.93"
license = "MIT OR Apache-2.0"
repository = "https://github.com/vortex-api/vortex"

[workspace.dependencies]
# Domain (no external deps ideally, but serde for DTOs)
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4", "serde"] }

# Async runtime
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "sync", "time"] }

# HTTP client
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

# UI
slint = "1.9"

# Error handling
thiserror = "2.0"

# Testing
wiremock = "0.6"

[profile.release]
lto = true
codegen-units = 1
strip = true
```

---

## Domain Types (Rust)

### Crate: vortex-domain

```toml
# /vortex/crates/vortex-domain/Cargo.toml
[package]
name = "vortex-domain"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
uuid = { workspace = true }
```

### File: src/lib.rs

```rust
// /vortex/crates/vortex-domain/src/lib.rs

//! Vortex Domain Layer
//!
//! Pure domain types with no external dependencies beyond serialization.
//! These types represent the core business concepts of the API client.

pub mod request;
pub mod response;
pub mod state;

pub use request::*;
pub use response::*;
pub use state::*;
```

### File: src/request.rs

```rust
// /vortex/crates/vortex-domain/src/request.rs

//! HTTP Request domain types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// HTTP methods supported by Vortex.
///
/// Covers all standard HTTP/1.1 methods plus PATCH.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    #[default]
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
    Trace,
}

impl HttpMethod {
    /// Returns the method as an uppercase string slice.
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Trace => "TRACE",
        }
    }

    /// Returns all available HTTP methods.
    pub fn all() -> &'static [HttpMethod] {
        &[
            HttpMethod::Get,
            HttpMethod::Post,
            HttpMethod::Put,
            HttpMethod::Patch,
            HttpMethod::Delete,
            HttpMethod::Head,
            HttpMethod::Options,
            HttpMethod::Trace,
        ]
    }

    /// Creates an HttpMethod from a string, case-insensitive.
    pub fn from_str_case_insensitive(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Some(HttpMethod::Get),
            "POST" => Some(HttpMethod::Post),
            "PUT" => Some(HttpMethod::Put),
            "PATCH" => Some(HttpMethod::Patch),
            "DELETE" => Some(HttpMethod::Delete),
            "HEAD" => Some(HttpMethod::Head),
            "OPTIONS" => Some(HttpMethod::Options),
            "TRACE" => Some(HttpMethod::Trace),
            _ => None,
        }
    }

    /// Returns true if this method typically has a request body.
    pub fn has_body(&self) -> bool {
        matches!(
            self,
            HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch
        )
    }
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Request body types supported by Vortex.
///
/// For Sprint 01, only `None`, `Text`, and `Json` are implemented.
/// Other variants are defined for future compatibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequestBody {
    /// No body (default for GET, DELETE, HEAD, OPTIONS).
    None,

    /// Raw text body with optional content type.
    Text {
        content: String,
        #[serde(default = "default_text_content_type")]
        content_type: String,
    },

    /// JSON body (serialized from a string for Sprint 01).
    Json {
        /// Raw JSON string. Validation happens at execution time.
        content: String,
    },

    /// Form URL-encoded body (future).
    #[serde(rename = "form_urlencoded")]
    FormUrlEncoded {
        fields: HashMap<String, String>,
    },

    /// Multipart form data (future).
    #[serde(rename = "form_data")]
    FormData {
        fields: Vec<FormDataField>,
    },

    /// Binary file body (future).
    Binary {
        path: String,
    },

    /// GraphQL query (future).
    GraphQL {
        query: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        variables: Option<serde_json::Value>,
    },
}

fn default_text_content_type() -> String {
    "text/plain".to_string()
}

impl Default for RequestBody {
    fn default() -> Self {
        RequestBody::None
    }
}

impl RequestBody {
    /// Creates a JSON body from a string.
    pub fn json(content: impl Into<String>) -> Self {
        RequestBody::Json {
            content: content.into(),
        }
    }

    /// Creates a text body with default content type.
    pub fn text(content: impl Into<String>) -> Self {
        RequestBody::Text {
            content: content.into(),
            content_type: default_text_content_type(),
        }
    }

    /// Returns true if the body is empty/none.
    pub fn is_none(&self) -> bool {
        matches!(self, RequestBody::None)
    }

    /// Returns the appropriate Content-Type header value for this body.
    pub fn content_type(&self) -> Option<&str> {
        match self {
            RequestBody::None => None,
            RequestBody::Text { content_type, .. } => Some(content_type),
            RequestBody::Json { .. } => Some("application/json"),
            RequestBody::FormUrlEncoded { .. } => Some("application/x-www-form-urlencoded"),
            RequestBody::FormData { .. } => None, // Set by multipart boundary
            RequestBody::Binary { .. } => Some("application/octet-stream"),
            RequestBody::GraphQL { .. } => Some("application/json"),
        }
    }
}

/// A single field in multipart form data (future use).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FormDataField {
    Text { name: String, value: String },
    File { name: String, path: String },
}

/// A key-value pair for headers or query parameters.
///
/// Supports enable/disable without deletion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyValuePair {
    pub key: String,
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

impl KeyValuePair {
    /// Creates a new enabled key-value pair.
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled: true,
            description: None,
        }
    }

    /// Creates a disabled key-value pair.
    pub fn disabled(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled: false,
            description: None,
        }
    }
}

/// Specification for an HTTP request.
///
/// This is the domain model representing a request to be executed.
/// It contains all information needed to make an HTTP call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestSpec {
    /// Unique identifier for this request.
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    /// Human-readable name (optional for Sprint 01).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// HTTP method.
    #[serde(default)]
    pub method: HttpMethod,

    /// Full URL including protocol (e.g., "https://api.example.com/users").
    pub url: String,

    /// HTTP headers to send.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<KeyValuePair>,

    /// Query parameters (appended to URL).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query_params: Vec<KeyValuePair>,

    /// Request body.
    #[serde(default)]
    pub body: RequestBody,

    /// Request timeout in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    30_000 // 30 seconds
}

impl Default for RequestSpec {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: None,
            method: HttpMethod::Get,
            url: String::new(),
            headers: Vec::new(),
            query_params: Vec::new(),
            body: RequestBody::None,
            timeout_ms: default_timeout_ms(),
        }
    }
}

impl RequestSpec {
    /// Creates a new GET request to the specified URL.
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: HttpMethod::Get,
            ..Default::default()
        }
    }

    /// Creates a new POST request to the specified URL.
    pub fn post(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: HttpMethod::Post,
            ..Default::default()
        }
    }

    /// Adds a header to the request.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push(KeyValuePair::new(key, value));
        self
    }

    /// Adds a query parameter to the request.
    pub fn with_query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query_params.push(KeyValuePair::new(key, value));
        self
    }

    /// Sets the request body.
    pub fn with_body(mut self, body: RequestBody) -> Self {
        self.body = body;
        self
    }

    /// Sets the timeout in milliseconds.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Returns only enabled headers.
    pub fn enabled_headers(&self) -> impl Iterator<Item = &KeyValuePair> {
        self.headers.iter().filter(|h| h.enabled)
    }

    /// Returns only enabled query parameters.
    pub fn enabled_query_params(&self) -> impl Iterator<Item = &KeyValuePair> {
        self.query_params.iter().filter(|q| q.enabled)
    }

    /// Builds the full URL with query parameters.
    pub fn full_url(&self) -> String {
        let enabled_params: Vec<_> = self.enabled_query_params().collect();
        if enabled_params.is_empty() {
            return self.url.clone();
        }

        let query_string = enabled_params
            .iter()
            .map(|p| format!("{}={}", urlencoding_key(&p.key), urlencoding_value(&p.value)))
            .collect::<Vec<_>>()
            .join("&");

        if self.url.contains('?') {
            format!("{}&{}", self.url, query_string)
        } else {
            format!("{}?{}", self.url, query_string)
        }
    }
}

// Simple URL encoding helpers (for Sprint 01, consider using `urlencoding` crate later)
fn urlencoding_key(s: &str) -> String {
    s.replace(' ', "+")
        .replace('&', "%26")
        .replace('=', "%3D")
}

fn urlencoding_value(s: &str) -> String {
    s.replace(' ', "+")
        .replace('&', "%26")
        .replace('=', "%3D")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_method_display() {
        assert_eq!(HttpMethod::Get.as_str(), "GET");
        assert_eq!(HttpMethod::Post.as_str(), "POST");
    }

    #[test]
    fn test_http_method_from_str() {
        assert_eq!(
            HttpMethod::from_str_case_insensitive("get"),
            Some(HttpMethod::Get)
        );
        assert_eq!(
            HttpMethod::from_str_case_insensitive("POST"),
            Some(HttpMethod::Post)
        );
        assert_eq!(HttpMethod::from_str_case_insensitive("invalid"), None);
    }

    #[test]
    fn test_request_spec_builder() {
        let request = RequestSpec::get("https://api.example.com/users")
            .with_header("Accept", "application/json")
            .with_query("page", "1")
            .with_timeout(5000);

        assert_eq!(request.method, HttpMethod::Get);
        assert_eq!(request.headers.len(), 1);
        assert_eq!(request.query_params.len(), 1);
        assert_eq!(request.timeout_ms, 5000);
    }

    #[test]
    fn test_full_url_with_query_params() {
        let request = RequestSpec::get("https://api.example.com/users")
            .with_query("page", "1")
            .with_query("limit", "10");

        assert_eq!(
            request.full_url(),
            "https://api.example.com/users?page=1&limit=10"
        );
    }

    #[test]
    fn test_body_content_type() {
        assert_eq!(RequestBody::None.content_type(), None);
        assert_eq!(RequestBody::json("{}").content_type(), Some("application/json"));
        assert_eq!(RequestBody::text("hello").content_type(), Some("text/plain"));
    }
}
```

### File: src/response.rs

```rust
// /vortex/crates/vortex-domain/src/response.rs

//! HTTP Response domain types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// HTTP status code with semantic helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusCode(pub u16);

impl StatusCode {
    /// Creates a new StatusCode.
    pub fn new(code: u16) -> Self {
        Self(code)
    }

    /// Returns the numeric status code.
    pub fn as_u16(&self) -> u16 {
        self.0
    }

    /// Returns true if this is a 1xx informational status.
    pub fn is_informational(&self) -> bool {
        (100..200).contains(&self.0)
    }

    /// Returns true if this is a 2xx success status.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.0)
    }

    /// Returns true if this is a 3xx redirection status.
    pub fn is_redirection(&self) -> bool {
        (300..400).contains(&self.0)
    }

    /// Returns true if this is a 4xx client error status.
    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.0)
    }

    /// Returns true if this is a 5xx server error status.
    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.0)
    }

    /// Returns true if this is any error status (4xx or 5xx).
    pub fn is_error(&self) -> bool {
        self.is_client_error() || self.is_server_error()
    }

    /// Returns the canonical reason phrase for common status codes.
    pub fn reason_phrase(&self) -> &'static str {
        match self.0 {
            100 => "Continue",
            101 => "Switching Protocols",
            200 => "OK",
            201 => "Created",
            202 => "Accepted",
            204 => "No Content",
            301 => "Moved Permanently",
            302 => "Found",
            304 => "Not Modified",
            307 => "Temporary Redirect",
            308 => "Permanent Redirect",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            408 => "Request Timeout",
            409 => "Conflict",
            422 => "Unprocessable Entity",
            429 => "Too Many Requests",
            500 => "Internal Server Error",
            501 => "Not Implemented",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            _ => "Unknown",
        }
    }

    /// Returns a CSS-friendly color category for UI display.
    pub fn color_category(&self) -> StatusColorCategory {
        match self.0 {
            100..=199 => StatusColorCategory::Informational,
            200..=299 => StatusColorCategory::Success,
            300..=399 => StatusColorCategory::Redirection,
            400..=499 => StatusColorCategory::ClientError,
            500..=599 => StatusColorCategory::ServerError,
            _ => StatusColorCategory::Unknown,
        }
    }
}

impl std::fmt::Display for StatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.0, self.reason_phrase())
    }
}

impl From<u16> for StatusCode {
    fn from(code: u16) -> Self {
        Self(code)
    }
}

/// Color category for status code display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusColorCategory {
    Informational, // Blue
    Success,       // Green
    Redirection,   // Blue
    ClientError,   // Orange
    ServerError,   // Red
    Unknown,       // Gray
}

/// Specification for an HTTP response.
///
/// Contains all information received from an HTTP call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseSpec {
    /// HTTP status code.
    pub status: StatusCode,

    /// Response headers (case-insensitive keys stored as received).
    pub headers: HashMap<String, String>,

    /// Response body as raw bytes.
    ///
    /// Stored as bytes to handle binary responses correctly.
    /// Use `body_as_string()` for text conversion.
    #[serde(with = "serde_bytes_base64")]
    pub body: Vec<u8>,

    /// Total request duration (from send to response complete).
    #[serde(with = "serde_duration_millis")]
    pub duration: Duration,

    /// Size of the response body in bytes.
    pub size_bytes: usize,

    /// Content-Type header value (extracted for convenience).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

impl ResponseSpec {
    /// Creates a new ResponseSpec.
    pub fn new(
        status: impl Into<StatusCode>,
        headers: HashMap<String, String>,
        body: Vec<u8>,
        duration: Duration,
    ) -> Self {
        let size_bytes = body.len();
        let content_type = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.clone());

        Self {
            status: status.into(),
            headers,
            body,
            duration,
            size_bytes,
            content_type,
        }
    }

    /// Attempts to convert the body to a UTF-8 string.
    ///
    /// Returns `None` if the body is not valid UTF-8.
    pub fn body_as_string(&self) -> Option<String> {
        String::from_utf8(self.body.clone()).ok()
    }

    /// Returns the body as a lossy UTF-8 string.
    ///
    /// Invalid UTF-8 sequences are replaced with the replacement character.
    pub fn body_as_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Attempts to parse the body as JSON.
    pub fn body_as_json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.body).ok()
    }

    /// Returns true if the content type indicates JSON.
    pub fn is_json(&self) -> bool {
        self.content_type
            .as_ref()
            .map(|ct| ct.contains("application/json") || ct.contains("+json"))
            .unwrap_or(false)
    }

    /// Returns true if the content type indicates text.
    pub fn is_text(&self) -> bool {
        self.content_type
            .as_ref()
            .map(|ct| ct.starts_with("text/") || ct.contains("xml") || self.is_json())
            .unwrap_or(false)
    }

    /// Returns a human-readable size string (e.g., "1.2 KB").
    pub fn size_display(&self) -> String {
        format_bytes(self.size_bytes)
    }

    /// Returns a human-readable duration string (e.g., "124 ms").
    pub fn duration_display(&self) -> String {
        let millis = self.duration.as_millis();
        if millis < 1000 {
            format!("{} ms", millis)
        } else {
            format!("{:.2} s", self.duration.as_secs_f64())
        }
    }

    /// Gets a header value by name (case-insensitive).
    pub fn get_header(&self, name: &str) -> Option<&String> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }
}

/// Formats bytes into a human-readable string.
fn format_bytes(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;
    const GB: usize = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// Custom serde for Duration as milliseconds
mod serde_duration_millis {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_millis().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

// Custom serde for Vec<u8> as base64
mod serde_bytes_base64 {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        STANDARD.encode(bytes).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_code_categories() {
        assert!(StatusCode::new(200).is_success());
        assert!(StatusCode::new(404).is_client_error());
        assert!(StatusCode::new(500).is_server_error());
        assert!(StatusCode::new(301).is_redirection());
    }

    #[test]
    fn test_status_code_display() {
        assert_eq!(StatusCode::new(200).to_string(), "200 OK");
        assert_eq!(StatusCode::new(404).to_string(), "404 Not Found");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
    }

    #[test]
    fn test_response_body_as_string() {
        let response = ResponseSpec::new(
            200,
            HashMap::new(),
            b"Hello, World!".to_vec(),
            Duration::from_millis(100),
        );

        assert_eq!(response.body_as_string(), Some("Hello, World!".to_string()));
    }

    #[test]
    fn test_response_is_json() {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = ResponseSpec::new(200, headers, vec![], Duration::from_millis(100));

        assert!(response.is_json());
        assert!(response.is_text());
    }
}
```

### File: src/state.rs

```rust
// /vortex/crates/vortex-domain/src/state.rs

//! Request execution state types for UI binding.

use crate::response::ResponseSpec;
use serde::{Deserialize, Serialize};

/// Represents the current state of a request in the UI.
///
/// This enum enables the UI to show appropriate feedback:
/// - `Idle`: Ready to send, show Send button
/// - `Loading`: Request in flight, show spinner and Cancel
/// - `Success`: Response received, show response panel
/// - `Error`: Request failed, show error message
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RequestState {
    /// No request has been sent yet, or reset after cancel.
    Idle,

    /// Request is in progress.
    Loading {
        /// When the request started (for elapsed time display).
        #[serde(skip)]
        started_at: Option<std::time::Instant>,
    },

    /// Request completed successfully.
    Success {
        /// The response data.
        response: ResponseSpec,
    },

    /// Request failed with an error.
    Error {
        /// Error category for display.
        kind: RequestErrorKind,
        /// Human-readable error message.
        message: String,
        /// Optional technical details.
        details: Option<String>,
    },
}

impl Default for RequestState {
    fn default() -> Self {
        Self::Idle
    }
}

impl RequestState {
    /// Creates a new Loading state with the current timestamp.
    pub fn loading() -> Self {
        Self::Loading {
            started_at: Some(std::time::Instant::now()),
        }
    }

    /// Creates a Success state from a response.
    pub fn success(response: ResponseSpec) -> Self {
        Self::Success { response }
    }

    /// Creates an Error state.
    pub fn error(kind: RequestErrorKind, message: impl Into<String>) -> Self {
        Self::Error {
            kind,
            message: message.into(),
            details: None,
        }
    }

    /// Creates an Error state with details.
    pub fn error_with_details(
        kind: RequestErrorKind,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self::Error {
            kind,
            message: message.into(),
            details: Some(details.into()),
        }
    }

    /// Returns true if the state is Idle.
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Returns true if a request is in progress.
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    /// Returns true if the last request succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Returns true if the last request failed.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Returns the response if in Success state.
    pub fn response(&self) -> Option<&ResponseSpec> {
        match self {
            Self::Success { response } => Some(response),
            _ => None,
        }
    }

    /// Returns the elapsed time if loading.
    pub fn elapsed(&self) -> Option<std::time::Duration> {
        match self {
            Self::Loading { started_at: Some(t) } => Some(t.elapsed()),
            _ => None,
        }
    }
}

/// Categories of request errors for user-friendly display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestErrorKind {
    /// Invalid URL format.
    InvalidUrl,

    /// DNS resolution failed.
    DnsError,

    /// Could not establish connection.
    ConnectionFailed,

    /// Connection was refused by the server.
    ConnectionRefused,

    /// Request timed out.
    Timeout,

    /// TLS/SSL error.
    TlsError,

    /// Invalid request body (e.g., malformed JSON).
    InvalidBody,

    /// Too many redirects.
    TooManyRedirects,

    /// Request was cancelled by user.
    Cancelled,

    /// Unknown or unexpected error.
    Unknown,
}

impl RequestErrorKind {
    /// Returns user-friendly suggestions for this error type.
    pub fn suggestions(&self) -> &[&'static str] {
        match self {
            Self::InvalidUrl => &[
                "Check that the URL starts with http:// or https://",
                "Verify there are no typos in the URL",
            ],
            Self::DnsError => &[
                "Check if the hostname is correct",
                "Verify your internet connection",
                "Try using an IP address instead",
            ],
            Self::ConnectionFailed | Self::ConnectionRefused => &[
                "Check if the server is running",
                "Verify the port number is correct",
                "Check your firewall settings",
            ],
            Self::Timeout => &[
                "The server may be slow or overloaded",
                "Try increasing the timeout value",
                "Check your network connection",
            ],
            Self::TlsError => &[
                "The server's SSL certificate may be invalid",
                "Check if the certificate has expired",
                "Verify the hostname matches the certificate",
            ],
            Self::InvalidBody => &[
                "Check that the JSON syntax is valid",
                "Verify all required fields are present",
            ],
            Self::TooManyRedirects => &[
                "The server may have a redirect loop",
                "Try the final URL directly",
            ],
            Self::Cancelled => &["Request was cancelled"],
            Self::Unknown => &[
                "An unexpected error occurred",
                "Check the error details for more information",
            ],
        }
    }

    /// Returns a human-readable title for this error type.
    pub fn title(&self) -> &'static str {
        match self {
            Self::InvalidUrl => "Invalid URL",
            Self::DnsError => "DNS Resolution Failed",
            Self::ConnectionFailed => "Connection Failed",
            Self::ConnectionRefused => "Connection Refused",
            Self::Timeout => "Request Timeout",
            Self::TlsError => "SSL/TLS Error",
            Self::InvalidBody => "Invalid Request Body",
            Self::TooManyRedirects => "Too Many Redirects",
            Self::Cancelled => "Request Cancelled",
            Self::Unknown => "Unknown Error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_state_transitions() {
        let state = RequestState::Idle;
        assert!(state.is_idle());

        let state = RequestState::loading();
        assert!(state.is_loading());

        let response = ResponseSpec::new(
            200,
            std::collections::HashMap::new(),
            vec![],
            std::time::Duration::from_millis(100),
        );
        let state = RequestState::success(response);
        assert!(state.is_success());
        assert!(state.response().is_some());

        let state = RequestState::error(RequestErrorKind::Timeout, "Request timed out");
        assert!(state.is_error());
    }

    #[test]
    fn test_error_kind_suggestions() {
        let suggestions = RequestErrorKind::ConnectionRefused.suggestions();
        assert!(!suggestions.is_empty());
    }
}
```

---

## Application Layer

### Crate: vortex-application

```toml
# /vortex/crates/vortex-application/Cargo.toml
[package]
name = "vortex-application"
version.workspace = true
edition.workspace = true

[dependencies]
vortex-domain = { path = "../vortex-domain" }
tokio = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
```

### File: src/lib.rs

```rust
// /vortex/crates/vortex-application/src/lib.rs

//! Vortex Application Layer
//!
//! Contains use cases and orchestration logic.
//! Defines ports (traits) that infrastructure must implement.

pub mod ports;
pub mod execute_request;

pub use ports::*;
pub use execute_request::*;
```

### File: src/ports.rs

```rust
// /vortex/crates/vortex-application/src/ports.rs

//! Port definitions (interfaces) for infrastructure adapters.
//!
//! These traits define what the application layer needs from infrastructure.
//! Following hexagonal architecture, the application layer depends on these
//! abstractions, not concrete implementations.

use std::future::Future;
use std::pin::Pin;
use vortex_domain::{RequestSpec, ResponseSpec};

/// Error type for HTTP client operations.
#[derive(Debug, thiserror::Error)]
pub enum HttpClientError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("DNS resolution failed for {host}: {message}")]
    DnsError { host: String, message: String },

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Connection refused by {host}:{port}")]
    ConnectionRefused { host: String, port: u16 },

    #[error("Request timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("TLS error: {0}")]
    TlsError(String),

    #[error("Invalid request body: {0}")]
    InvalidBody(String),

    #[error("Too many redirects (max: {max})")]
    TooManyRedirects { max: u32 },

    #[error("Request cancelled")]
    Cancelled,

    #[error("HTTP error: {0}")]
    Other(String),
}

impl HttpClientError {
    /// Converts this error to a domain RequestErrorKind.
    pub fn to_error_kind(&self) -> vortex_domain::RequestErrorKind {
        use vortex_domain::RequestErrorKind;
        match self {
            Self::InvalidUrl(_) => RequestErrorKind::InvalidUrl,
            Self::DnsError { .. } => RequestErrorKind::DnsError,
            Self::ConnectionFailed(_) => RequestErrorKind::ConnectionFailed,
            Self::ConnectionRefused { .. } => RequestErrorKind::ConnectionRefused,
            Self::Timeout { .. } => RequestErrorKind::Timeout,
            Self::TlsError(_) => RequestErrorKind::TlsError,
            Self::InvalidBody(_) => RequestErrorKind::InvalidBody,
            Self::TooManyRedirects { .. } => RequestErrorKind::TooManyRedirects,
            Self::Cancelled => RequestErrorKind::Cancelled,
            Self::Other(_) => RequestErrorKind::Unknown,
        }
    }
}

/// Port for HTTP client operations.
///
/// Infrastructure must implement this trait to provide HTTP functionality.
/// The application layer uses this abstraction to remain decoupled from
/// the specific HTTP library (reqwest, hyper, etc.).
pub trait HttpClient: Send + Sync {
    /// Executes an HTTP request and returns the response.
    ///
    /// This method is async and should handle:
    /// - URL construction with query parameters
    /// - Header setting
    /// - Body serialization
    /// - Timeout enforcement
    /// - Redirect following
    ///
    /// # Arguments
    /// * `request` - The request specification to execute
    ///
    /// # Returns
    /// * `Ok(ResponseSpec)` - The response on success
    /// * `Err(HttpClientError)` - Detailed error on failure
    fn execute(
        &self,
        request: &RequestSpec,
    ) -> Pin<Box<dyn Future<Output = Result<ResponseSpec, HttpClientError>> + Send + '_>>;
}

/// A cancellation token for aborting in-flight requests.
///
/// Used to implement the Cancel button in the UI.
#[derive(Clone)]
pub struct CancellationToken {
    inner: tokio::sync::watch::Sender<bool>,
}

impl CancellationToken {
    /// Creates a new cancellation token.
    pub fn new() -> (Self, CancellationReceiver) {
        let (tx, rx) = tokio::sync::watch::channel(false);
        (Self { inner: tx }, CancellationReceiver { inner: rx })
    }

    /// Signals cancellation.
    pub fn cancel(&self) {
        let _ = self.inner.send(true);
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new().0
    }
}

/// Receiver side of a cancellation token.
#[derive(Clone)]
pub struct CancellationReceiver {
    inner: tokio::sync::watch::Receiver<bool>,
}

impl CancellationReceiver {
    /// Returns true if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        *self.inner.borrow()
    }

    /// Waits until cancellation is requested.
    pub async fn cancelled(&mut self) {
        while !*self.inner.borrow() {
            if self.inner.changed().await.is_err() {
                break;
            }
        }
    }
}
```

### File: src/execute_request.rs

```rust
// /vortex/crates/vortex-application/src/execute_request.rs

//! Execute Request Use Case
//!
//! This is the primary use case for Sprint 01: executing an HTTP request
//! and returning the response or error.

use crate::ports::{CancellationReceiver, HttpClient, HttpClientError};
use std::sync::Arc;
use vortex_domain::{RequestSpec, RequestState, ResponseSpec};

/// Result type for request execution.
pub type ExecuteResult = Result<ResponseSpec, ExecuteRequestError>;

/// Error type for the execute request use case.
#[derive(Debug, thiserror::Error)]
pub enum ExecuteRequestError {
    #[error("URL is required")]
    EmptyUrl,

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("{0}")]
    HttpError(#[from] HttpClientError),
}

impl ExecuteRequestError {
    /// Converts this error to a RequestState::Error for UI display.
    pub fn to_request_state(&self) -> RequestState {
        match self {
            Self::EmptyUrl => RequestState::error(
                vortex_domain::RequestErrorKind::InvalidUrl,
                "URL is required",
            ),
            Self::InvalidUrl(msg) => RequestState::error(
                vortex_domain::RequestErrorKind::InvalidUrl,
                msg.clone(),
            ),
            Self::HttpError(e) => RequestState::error_with_details(
                e.to_error_kind(),
                e.to_error_kind().title(),
                e.to_string(),
            ),
        }
    }
}

/// Use case for executing HTTP requests.
///
/// This struct encapsulates the business logic for sending requests
/// and handling responses. It uses the HttpClient port for actual
/// HTTP communication.
///
/// # Example
///
/// ```ignore
/// let http_client = ReqwestHttpClient::new();
/// let use_case = ExecuteRequest::new(Arc::new(http_client));
///
/// let request = RequestSpec::get("https://api.example.com/users");
/// let response = use_case.execute(&request).await?;
/// ```
pub struct ExecuteRequest<C: HttpClient> {
    client: Arc<C>,
}

impl<C: HttpClient> ExecuteRequest<C> {
    /// Creates a new ExecuteRequest use case with the given HTTP client.
    pub fn new(client: Arc<C>) -> Self {
        Self { client }
    }

    /// Executes the request and returns the result.
    ///
    /// # Validation
    /// - URL must not be empty
    /// - URL must be parseable (basic validation)
    ///
    /// # Errors
    /// Returns `ExecuteRequestError` on validation or HTTP failures.
    pub async fn execute(&self, request: &RequestSpec) -> ExecuteResult {
        // Validate request
        self.validate(request)?;

        // Execute via HTTP client
        let response = self.client.execute(request).await?;

        Ok(response)
    }

    /// Executes the request with cancellation support.
    ///
    /// # Arguments
    /// * `request` - The request to execute
    /// * `cancel` - Cancellation receiver for aborting the request
    ///
    /// # Returns
    /// The response, or an error if cancelled or failed.
    pub async fn execute_with_cancellation(
        &self,
        request: &RequestSpec,
        mut cancel: CancellationReceiver,
    ) -> ExecuteResult {
        // Validate request
        self.validate(request)?;

        // Race between execution and cancellation
        tokio::select! {
            result = self.client.execute(request) => {
                result.map_err(ExecuteRequestError::from)
            }
            _ = cancel.cancelled() => {
                Err(ExecuteRequestError::HttpError(HttpClientError::Cancelled))
            }
        }
    }

    /// Validates the request before execution.
    fn validate(&self, request: &RequestSpec) -> Result<(), ExecuteRequestError> {
        // Check for empty URL
        if request.url.trim().is_empty() {
            return Err(ExecuteRequestError::EmptyUrl);
        }

        // Basic URL validation
        if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
            return Err(ExecuteRequestError::InvalidUrl(
                "URL must start with http:// or https://".to_string(),
            ));
        }

        Ok(())
    }
}

/// Extension trait for convenient RequestState conversion.
pub trait ExecuteResultExt {
    /// Converts the result to a RequestState for UI binding.
    fn to_request_state(self) -> RequestState;
}

impl ExecuteResultExt for ExecuteResult {
    fn to_request_state(self) -> RequestState {
        match self {
            Ok(response) => RequestState::success(response),
            Err(e) => e.to_request_state(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::future::Future;
    use std::pin::Pin;
    use std::time::Duration;

    /// Mock HTTP client for testing.
    struct MockHttpClient {
        response: Result<ResponseSpec, HttpClientError>,
    }

    impl MockHttpClient {
        fn success() -> Self {
            Self {
                response: Ok(ResponseSpec::new(
                    200,
                    HashMap::new(),
                    b"OK".to_vec(),
                    Duration::from_millis(50),
                )),
            }
        }

        fn error(err: HttpClientError) -> Self {
            Self { response: Err(err) }
        }
    }

    impl HttpClient for MockHttpClient {
        fn execute(
            &self,
            _request: &RequestSpec,
        ) -> Pin<Box<dyn Future<Output = Result<ResponseSpec, HttpClientError>> + Send + '_>>
        {
            let result = self.response.clone();
            Box::pin(async move { result })
        }
    }

    #[tokio::test]
    async fn test_execute_success() {
        let client = Arc::new(MockHttpClient::success());
        let use_case = ExecuteRequest::new(client);

        let request = RequestSpec::get("https://api.example.com/test");
        let result = use_case.execute(&request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status.as_u16(), 200);
    }

    #[tokio::test]
    async fn test_execute_empty_url() {
        let client = Arc::new(MockHttpClient::success());
        let use_case = ExecuteRequest::new(client);

        let request = RequestSpec {
            url: "".to_string(),
            ..Default::default()
        };
        let result = use_case.execute(&request).await;

        assert!(matches!(result, Err(ExecuteRequestError::EmptyUrl)));
    }

    #[tokio::test]
    async fn test_execute_invalid_url() {
        let client = Arc::new(MockHttpClient::success());
        let use_case = ExecuteRequest::new(client);

        let request = RequestSpec {
            url: "not-a-valid-url".to_string(),
            ..Default::default()
        };
        let result = use_case.execute(&request).await;

        assert!(matches!(result, Err(ExecuteRequestError::InvalidUrl(_))));
    }

    #[tokio::test]
    async fn test_execute_http_error() {
        let client = Arc::new(MockHttpClient::error(HttpClientError::Timeout {
            timeout_ms: 5000,
        }));
        let use_case = ExecuteRequest::new(client);

        let request = RequestSpec::get("https://api.example.com/test");
        let result = use_case.execute(&request).await;

        assert!(matches!(
            result,
            Err(ExecuteRequestError::HttpError(HttpClientError::Timeout { .. }))
        ));
    }

    #[tokio::test]
    async fn test_result_to_request_state() {
        use crate::execute_request::ExecuteResultExt;

        let success_result: ExecuteResult = Ok(ResponseSpec::new(
            200,
            HashMap::new(),
            vec![],
            Duration::from_millis(100),
        ));
        let state = success_result.to_request_state();
        assert!(state.is_success());

        let error_result: ExecuteResult = Err(ExecuteRequestError::EmptyUrl);
        let state = error_result.to_request_state();
        assert!(state.is_error());
    }
}
```

---

## Infrastructure Layer

### Crate: vortex-infrastructure

```toml
# /vortex/crates/vortex-infrastructure/Cargo.toml
[package]
name = "vortex-infrastructure"
version.workspace = true
edition.workspace = true

[dependencies]
vortex-domain = { path = "../vortex-domain" }
vortex-application = { path = "../vortex-application" }
reqwest = { workspace = true }
tokio = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
wiremock = { workspace = true }
```

### File: src/lib.rs

```rust
// /vortex/crates/vortex-infrastructure/src/lib.rs

//! Vortex Infrastructure Layer
//!
//! Concrete implementations of application ports.
//! Contains adapters for HTTP, storage, and external services.

pub mod http_client;

pub use http_client::ReqwestHttpClient;
```

### File: src/http_client.rs

```rust
// /vortex/crates/vortex-infrastructure/src/http_client.rs

//! HTTP Client implementation using reqwest.
//!
//! This adapter implements the HttpClient port using the reqwest library.
//! It handles all HTTP communication for the application.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use reqwest::{Client, Method, Url};
use vortex_application::ports::{HttpClient, HttpClientError};
use vortex_domain::{HttpMethod, RequestBody, RequestSpec, ResponseSpec};

/// HTTP client implementation using reqwest.
///
/// This is the primary HTTP adapter for Vortex. It wraps reqwest::Client
/// and implements the HttpClient port from the application layer.
pub struct ReqwestHttpClient {
    client: Client,
}

impl ReqwestHttpClient {
    /// Creates a new HTTP client with default settings.
    ///
    /// Default configuration:
    /// - Connection timeout: 30 seconds
    /// - Follow redirects: up to 10
    /// - TLS verification: enabled
    /// - User-Agent: "Vortex/0.1.0"
    pub fn new() -> Result<Self, HttpClientError> {
        let client = Client::builder()
            .user_agent("Vortex/0.1.0")
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| HttpClientError::Other(e.to_string()))?;

        Ok(Self { client })
    }

    /// Creates a new HTTP client with custom configuration.
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    /// Converts domain HttpMethod to reqwest Method.
    fn to_reqwest_method(method: HttpMethod) -> Method {
        match method {
            HttpMethod::Get => Method::GET,
            HttpMethod::Post => Method::POST,
            HttpMethod::Put => Method::PUT,
            HttpMethod::Patch => Method::PATCH,
            HttpMethod::Delete => Method::DELETE,
            HttpMethod::Head => Method::HEAD,
            HttpMethod::Options => Method::OPTIONS,
            HttpMethod::Trace => Method::TRACE,
        }
    }

    /// Builds the request body from domain RequestBody.
    fn build_body(
        &self,
        builder: reqwest::RequestBuilder,
        body: &RequestBody,
    ) -> Result<reqwest::RequestBuilder, HttpClientError> {
        match body {
            RequestBody::None => Ok(builder),

            RequestBody::Text { content, .. } => Ok(builder.body(content.clone())),

            RequestBody::Json { content } => {
                // Validate JSON syntax
                let _: serde_json::Value = serde_json::from_str(content)
                    .map_err(|e| HttpClientError::InvalidBody(format!("Invalid JSON: {}", e)))?;
                Ok(builder.body(content.clone()))
            }

            RequestBody::FormUrlEncoded { fields } => Ok(builder.form(fields)),

            // Future body types return error for now
            RequestBody::FormData { .. } => Err(HttpClientError::Other(
                "Multipart form data not yet implemented".to_string(),
            )),

            RequestBody::Binary { path } => Err(HttpClientError::Other(format!(
                "Binary body not yet implemented (path: {})",
                path
            ))),

            RequestBody::GraphQL { query, variables } => {
                let body = serde_json::json!({
                    "query": query,
                    "variables": variables,
                });
                Ok(builder.json(&body))
            }
        }
    }

    /// Maps reqwest errors to domain HttpClientError.
    fn map_error(&self, error: reqwest::Error, timeout_ms: u64) -> HttpClientError {
        if error.is_timeout() {
            return HttpClientError::Timeout { timeout_ms };
        }

        if error.is_connect() {
            let message = error.to_string();
            if message.contains("dns") || message.contains("resolve") {
                return HttpClientError::DnsError {
                    host: error
                        .url()
                        .map(|u| u.host_str().unwrap_or("unknown").to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    message,
                };
            }
            if message.contains("refused") {
                return HttpClientError::ConnectionRefused {
                    host: error
                        .url()
                        .map(|u| u.host_str().unwrap_or("unknown").to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    port: error
                        .url()
                        .and_then(|u| u.port())
                        .unwrap_or(80),
                };
            }
            return HttpClientError::ConnectionFailed(message);
        }

        if error.is_redirect() {
            return HttpClientError::TooManyRedirects { max: 10 };
        }

        HttpClientError::Other(error.to_string())
    }
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default HTTP client")
    }
}

impl HttpClient for ReqwestHttpClient {
    fn execute(
        &self,
        request: &RequestSpec,
    ) -> Pin<Box<dyn Future<Output = Result<ResponseSpec, HttpClientError>> + Send + '_>> {
        // Clone what we need to move into the async block
        let method = request.method;
        let url = request.full_url();
        let headers = request.headers.clone();
        let body = request.body.clone();
        let timeout_ms = request.timeout_ms;

        Box::pin(async move {
            // Parse URL
            let parsed_url = Url::parse(&url).map_err(|e| {
                HttpClientError::InvalidUrl(format!("{}: {}", e, url))
            })?;

            // Start timing
            let start = Instant::now();

            // Build request
            let mut builder = self
                .client
                .request(Self::to_reqwest_method(method), parsed_url)
                .timeout(Duration::from_millis(timeout_ms));

            // Add headers
            for header in headers.iter().filter(|h| h.enabled) {
                builder = builder.header(&header.key, &header.value);
            }

            // Add Content-Type if body has one and not already set
            if let Some(content_type) = body.content_type() {
                let has_content_type = headers
                    .iter()
                    .any(|h| h.enabled && h.key.eq_ignore_ascii_case("content-type"));
                if !has_content_type {
                    builder = builder.header("Content-Type", content_type);
                }
            }

            // Add body
            builder = self.build_body(builder, &body)?;

            // Execute request
            let response = builder
                .send()
                .await
                .map_err(|e| self.map_error(e, timeout_ms))?;

            // Calculate duration
            let duration = start.elapsed();

            // Extract response data
            let status = response.status().as_u16();

            // Collect headers
            let response_headers: HashMap<String, String> = response
                .headers()
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        v.to_str().unwrap_or("<binary>").to_string(),
                    )
                })
                .collect();

            // Read body
            let body_bytes = response
                .bytes()
                .await
                .map_err(|e| HttpClientError::Other(format!("Failed to read body: {}", e)))?
                .to_vec();

            Ok(ResponseSpec::new(status, response_headers, body_bytes, duration))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_simple_get_request() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"status":"ok"}"#)
                    .insert_header("Content-Type", "application/json"),
            )
            .mount(&mock_server)
            .await;

        let client = ReqwestHttpClient::new().unwrap();
        let request = RequestSpec::get(format!("{}/test", mock_server.uri()));

        let response = client.execute(&request).await.unwrap();

        assert_eq!(response.status.as_u16(), 200);
        assert!(response.is_json());
        assert_eq!(response.body_as_string(), Some(r#"{"status":"ok"}"#.to_string()));
    }

    #[tokio::test]
    async fn test_post_with_json_body() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/users"))
            .respond_with(ResponseTemplate::new(201).set_body_string(r#"{"id":1}"#))
            .mount(&mock_server)
            .await;

        let client = ReqwestHttpClient::new().unwrap();
        let request = RequestSpec::post(format!("{}/users", mock_server.uri()))
            .with_body(RequestBody::json(r#"{"name":"test"}"#));

        let response = client.execute(&request).await.unwrap();

        assert_eq!(response.status.as_u16(), 201);
    }

    #[tokio::test]
    async fn test_request_with_headers() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/auth"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = ReqwestHttpClient::new().unwrap();
        let request = RequestSpec::get(format!("{}/auth", mock_server.uri()))
            .with_header("Authorization", "Bearer token123")
            .with_header("Accept", "application/json");

        let response = client.execute(&request).await.unwrap();

        assert_eq!(response.status.as_u16(), 200);
    }

    #[tokio::test]
    async fn test_request_with_query_params() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = ReqwestHttpClient::new().unwrap();
        let request = RequestSpec::get(format!("{}/search", mock_server.uri()))
            .with_query("q", "rust")
            .with_query("page", "1");

        let response = client.execute(&request).await.unwrap();

        assert_eq!(response.status.as_u16(), 200);
    }

    #[tokio::test]
    async fn test_invalid_json_body() {
        let client = ReqwestHttpClient::new().unwrap();
        let request = RequestSpec::post("https://api.example.com/test")
            .with_body(RequestBody::json("{invalid json}"));

        let result = client.execute(&request).await;

        assert!(matches!(result, Err(HttpClientError::InvalidBody(_))));
    }
}
```

---

## UI Layer (Slint)

### Crate: vortex-ui

```toml
# /vortex/crates/vortex-ui/Cargo.toml
[package]
name = "vortex-ui"
version.workspace = true
edition.workspace = true

[dependencies]
vortex-domain = { path = "../vortex-domain" }
vortex-application = { path = "../vortex-application" }
vortex-infrastructure = { path = "../vortex-infrastructure" }
slint = { workspace = true }
tokio = { workspace = true }

[build-dependencies]
slint-build = "1.9"
```

### File: build.rs

```rust
// /vortex/crates/vortex-ui/build.rs

fn main() {
    slint_build::compile("ui/main.slint").unwrap();
}
```

### File: ui/theme.slint

```slint
// /vortex/crates/vortex-ui/ui/theme.slint

// Vortex Theme - Dark Mode
// Based on VS Code dark theme colors

export global VortexPalette {
    // Backgrounds
    out property <color> bg-primary: #1e1e1e;
    out property <color> bg-secondary: #252526;
    out property <color> bg-tertiary: #2d2d2d;
    out property <color> bg-input: #3c3c3c;
    out property <color> bg-hover: #094771;
    out property <color> bg-selected: #094771;

    // Text
    out property <color> text-primary: #cccccc;
    out property <color> text-secondary: #858585;
    out property <color> text-accent: #4fc1ff;
    out property <color> text-placeholder: #6b6b6b;

    // Status colors
    out property <color> status-success: #4ec9b0;
    out property <color> status-info: #569cd6;
    out property <color> status-warning: #ce9178;
    out property <color> status-error: #f14c4c;

    // Method colors
    out property <color> method-get: #61affe;
    out property <color> method-post: #49cc90;
    out property <color> method-put: #fca130;
    out property <color> method-patch: #50e3c2;
    out property <color> method-delete: #f93e3e;
    out property <color> method-head: #9012fe;
    out property <color> method-options: #0d5aa7;

    // Borders
    out property <color> border-default: #3c3c3c;
    out property <color> border-focus: #007acc;

    // Buttons
    out property <color> button-primary: #0e639c;
    out property <color> button-primary-hover: #1177bb;
}

export global VortexTypography {
    out property <length> font-xs: 11px;
    out property <length> font-sm: 12px;
    out property <length> font-base: 13px;
    out property <length> font-lg: 14px;
    out property <length> font-xl: 16px;

    out property <int> weight-normal: 400;
    out property <int> weight-medium: 500;
    out property <int> weight-bold: 600;
}

export global VortexSpacing {
    out property <length> xs: 4px;
    out property <length> sm: 8px;
    out property <length> md: 12px;
    out property <length> lg: 16px;
    out property <length> xl: 24px;
}
```

### File: ui/components/method_selector.slint

```slint
// /vortex/crates/vortex-ui/ui/components/method_selector.slint

import { ComboBox } from "std-widgets.slint";
import { VortexPalette, VortexTypography, VortexSpacing } from "../theme.slint";

export component MethodSelector inherits Rectangle {
    in-out property <int> current-index: 0;
    in property <[string]> methods: ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

    callback method-changed(string);

    width: 100px;
    height: 32px;

    Rectangle {
        background: VortexPalette.bg-input;
        border-radius: 4px;
        border-width: 1px;
        border-color: VortexPalette.border-default;

        HorizontalLayout {
            padding: VortexSpacing.sm;

            method-label := Text {
                text: methods[current-index];
                color: root.method-color(methods[current-index]);
                font-size: VortexTypography.font-base;
                font-weight: VortexTypography.weight-bold;
                vertical-alignment: center;
            }

            Text {
                text: "\u{25BC}";
                color: VortexPalette.text-secondary;
                font-size: 8px;
                vertical-alignment: center;
            }
        }

        TouchArea {
            clicked => {
                popup.show();
            }
        }
    }

    popup := PopupWindow {
        x: 0;
        y: root.height;
        width: root.width;

        Rectangle {
            background: VortexPalette.bg-secondary;
            border-radius: 4px;
            border-width: 1px;
            border-color: VortexPalette.border-default;
            drop-shadow-blur: 8px;
            drop-shadow-color: #00000080;

            VerticalLayout {
                for method[index] in methods: Rectangle {
                    height: 28px;
                    background: touch.has-hover ? VortexPalette.bg-hover : transparent;

                    HorizontalLayout {
                        padding-left: VortexSpacing.sm;
                        padding-right: VortexSpacing.sm;

                        Text {
                            text: method;
                            color: root.method-color(method);
                            font-size: VortexTypography.font-base;
                            font-weight: VortexTypography.weight-bold;
                            vertical-alignment: center;
                        }
                    }

                    touch := TouchArea {
                        clicked => {
                            current-index = index;
                            method-changed(method);
                            popup.close();
                        }
                    }
                }
            }
        }
    }

    pure function method-color(method: string) -> color {
        if method == "GET" { return VortexPalette.method-get; }
        if method == "POST" { return VortexPalette.method-post; }
        if method == "PUT" { return VortexPalette.method-put; }
        if method == "PATCH" { return VortexPalette.method-patch; }
        if method == "DELETE" { return VortexPalette.method-delete; }
        if method == "HEAD" { return VortexPalette.method-head; }
        if method == "OPTIONS" { return VortexPalette.method-options; }
        return VortexPalette.text-primary;
    }
}
```

### File: ui/components/url_bar.slint

```slint
// /vortex/crates/vortex-ui/ui/components/url_bar.slint

import { LineEdit, Button } from "std-widgets.slint";
import { VortexPalette, VortexTypography, VortexSpacing } from "../theme.slint";
import { MethodSelector } from "method_selector.slint";

export component UrlBar inherits Rectangle {
    in-out property <string> url: "";
    in-out property <int> method-index: 0;
    in property <bool> is-loading: false;
    in property <[string]> methods: ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

    callback send-clicked();
    callback cancel-clicked();
    callback method-changed(string);
    callback url-changed(string);

    height: 48px;
    background: VortexPalette.bg-secondary;

    HorizontalLayout {
        padding: VortexSpacing.sm;
        spacing: VortexSpacing.sm;

        MethodSelector {
            current-index <=> method-index;
            methods: root.methods;
            method-changed(m) => { root.method-changed(m); }
        }

        Rectangle {
            horizontal-stretch: 1;
            background: VortexPalette.bg-input;
            border-radius: 4px;
            border-width: 1px;
            border-color: url-input.has-focus ? VortexPalette.border-focus : VortexPalette.border-default;

            url-input := LineEdit {
                text <=> url;
                placeholder-text: "Enter URL (e.g., https://api.example.com/users)";
                font-size: VortexTypography.font-base;

                accepted => {
                    if !is-loading {
                        send-clicked();
                    }
                }

                edited => {
                    url-changed(self.text);
                }
            }
        }

        if !is-loading: Rectangle {
            width: 80px;
            height: 32px;
            background: VortexPalette.button-primary;
            border-radius: 4px;

            states [
                hover when send-touch.has-hover: {
                    background: VortexPalette.button-primary-hover;
                }
            ]

            Text {
                text: "Send";
                color: white;
                font-size: VortexTypography.font-base;
                font-weight: VortexTypography.weight-medium;
                horizontal-alignment: center;
                vertical-alignment: center;
            }

            send-touch := TouchArea {
                clicked => {
                    send-clicked();
                }
            }
        }

        if is-loading: Rectangle {
            width: 80px;
            height: 32px;
            background: VortexPalette.status-error;
            border-radius: 4px;

            states [
                hover when cancel-touch.has-hover: {
                    background: #d43d3d;
                }
            ]

            Text {
                text: "Cancel";
                color: white;
                font-size: VortexTypography.font-base;
                font-weight: VortexTypography.weight-medium;
                horizontal-alignment: center;
                vertical-alignment: center;
            }

            cancel-touch := TouchArea {
                clicked => {
                    cancel-clicked();
                }
            }
        }
    }
}
```

### File: ui/components/response_panel.slint

```slint
// /vortex/crates/vortex-ui/ui/components/response_panel.slint

import { TextEdit, Button, ScrollView } from "std-widgets.slint";
import { VortexPalette, VortexTypography, VortexSpacing } from "../theme.slint";

// State enum values (must match Rust side)
// 0 = Idle, 1 = Loading, 2 = Success, 3 = Error

export component ResponsePanel inherits Rectangle {
    // State: 0=Idle, 1=Loading, 2=Success, 3=Error
    in property <int> state: 0;

    // Response data (for Success state)
    in property <int> status-code: 0;
    in property <string> status-text: "";
    in property <string> response-body: "";
    in property <string> duration: "";
    in property <string> size: "";

    // Error data (for Error state)
    in property <string> error-title: "";
    in property <string> error-message: "";
    in property <[string]> error-suggestions: [];

    // Loading data
    in property <string> elapsed-time: "0ms";

    callback retry-clicked();
    callback copy-body-clicked();

    background: VortexPalette.bg-secondary;

    VerticalLayout {
        // Idle state - show placeholder
        if state == 0: Rectangle {
            vertical-stretch: 1;

            VerticalLayout {
                alignment: center;
                spacing: VortexSpacing.md;

                Text {
                    text: "No Response";
                    color: VortexPalette.text-secondary;
                    font-size: VortexTypography.font-xl;
                    horizontal-alignment: center;
                }

                Text {
                    text: "Enter a URL and click Send to make a request";
                    color: VortexPalette.text-secondary;
                    font-size: VortexTypography.font-base;
                    horizontal-alignment: center;
                }

                Text {
                    text: "Ctrl+Enter to send";
                    color: VortexPalette.text-secondary;
                    font-size: VortexTypography.font-sm;
                    horizontal-alignment: center;
                }
            }
        }

        // Loading state - show spinner/progress
        if state == 1: Rectangle {
            vertical-stretch: 1;

            VerticalLayout {
                alignment: center;
                spacing: VortexSpacing.lg;

                // Simple loading indicator
                Rectangle {
                    width: 48px;
                    height: 48px;
                    border-radius: 24px;
                    border-width: 3px;
                    border-color: VortexPalette.button-primary;

                    // Animated spinner effect using rotation
                    Rectangle {
                        width: 12px;
                        height: 12px;
                        x: 18px;
                        y: -3px;
                        background: VortexPalette.button-primary;
                        border-radius: 6px;
                    }
                }

                Text {
                    text: "Sending request...";
                    color: VortexPalette.text-primary;
                    font-size: VortexTypography.font-lg;
                    horizontal-alignment: center;
                }

                Text {
                    text: elapsed-time;
                    color: VortexPalette.text-secondary;
                    font-size: VortexTypography.font-base;
                    horizontal-alignment: center;
                }
            }
        }

        // Success state - show response
        if state == 2: VerticalLayout {
            spacing: 0;

            // Status bar
            Rectangle {
                height: 40px;
                background: VortexPalette.bg-tertiary;
                border-width: 0 0 1px 0;
                border-color: VortexPalette.border-default;

                HorizontalLayout {
                    padding: VortexSpacing.sm;
                    spacing: VortexSpacing.lg;

                    // Status indicator
                    HorizontalLayout {
                        spacing: VortexSpacing.xs;

                        Rectangle {
                            width: 8px;
                            height: 8px;
                            border-radius: 4px;
                            background: root.status-color();
                        }

                        Text {
                            text: status-code + " " + status-text;
                            color: root.status-color();
                            font-size: VortexTypography.font-base;
                            font-weight: VortexTypography.weight-bold;
                            vertical-alignment: center;
                        }
                    }

                    // Duration
                    HorizontalLayout {
                        spacing: VortexSpacing.xs;

                        Text {
                            text: "Time:";
                            color: VortexPalette.text-secondary;
                            font-size: VortexTypography.font-sm;
                            vertical-alignment: center;
                        }

                        Text {
                            text: duration;
                            color: VortexPalette.text-primary;
                            font-size: VortexTypography.font-sm;
                            vertical-alignment: center;
                        }
                    }

                    // Size
                    HorizontalLayout {
                        spacing: VortexSpacing.xs;

                        Text {
                            text: "Size:";
                            color: VortexPalette.text-secondary;
                            font-size: VortexTypography.font-sm;
                            vertical-alignment: center;
                        }

                        Text {
                            text: size;
                            color: VortexPalette.text-primary;
                            font-size: VortexTypography.font-sm;
                            vertical-alignment: center;
                        }
                    }

                    Rectangle { horizontal-stretch: 1; }

                    // Copy button
                    Rectangle {
                        width: 60px;
                        height: 24px;
                        background: copy-touch.has-hover ? VortexPalette.bg-hover : transparent;
                        border-radius: 4px;

                        Text {
                            text: "Copy";
                            color: VortexPalette.text-accent;
                            font-size: VortexTypography.font-sm;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }

                        copy-touch := TouchArea {
                            clicked => { copy-body-clicked(); }
                        }
                    }
                }
            }

            // Response body
            Rectangle {
                vertical-stretch: 1;
                background: VortexPalette.bg-primary;

                ScrollView {
                    viewport-width: self.width;

                    body-text := TextEdit {
                        text: response-body;
                        read-only: true;
                        font-size: VortexTypography.font-base;
                        // Note: Slint TextEdit styling is limited
                        // For syntax highlighting, consider custom rendering
                    }
                }
            }
        }

        // Error state - show error details
        if state == 3: Rectangle {
            vertical-stretch: 1;

            VerticalLayout {
                alignment: center;
                spacing: VortexSpacing.lg;
                padding: VortexSpacing.xl;

                // Error icon
                Rectangle {
                    width: 48px;
                    height: 48px;
                    border-radius: 24px;
                    background: #f14c4c20;

                    Text {
                        text: "\u{26A0}";
                        color: VortexPalette.status-error;
                        font-size: 24px;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }
                }

                // Error title
                Text {
                    text: error-title;
                    color: VortexPalette.status-error;
                    font-size: VortexTypography.font-xl;
                    font-weight: VortexTypography.weight-bold;
                    horizontal-alignment: center;
                }

                // Error message
                Text {
                    text: error-message;
                    color: VortexPalette.text-primary;
                    font-size: VortexTypography.font-base;
                    horizontal-alignment: center;
                    wrap: word-wrap;
                }

                // Suggestions
                if error-suggestions.length > 0: Rectangle {
                    background: VortexPalette.bg-tertiary;
                    border-radius: 8px;

                    VerticalLayout {
                        padding: VortexSpacing.md;
                        spacing: VortexSpacing.sm;

                        Text {
                            text: "Suggestions:";
                            color: VortexPalette.text-secondary;
                            font-size: VortexTypography.font-sm;
                            font-weight: VortexTypography.weight-medium;
                        }

                        for suggestion in error-suggestions: HorizontalLayout {
                            spacing: VortexSpacing.sm;

                            Text {
                                text: "\u{2022}";
                                color: VortexPalette.text-secondary;
                                font-size: VortexTypography.font-base;
                            }

                            Text {
                                text: suggestion;
                                color: VortexPalette.text-primary;
                                font-size: VortexTypography.font-base;
                                wrap: word-wrap;
                            }
                        }
                    }
                }

                // Retry button
                Rectangle {
                    width: 100px;
                    height: 32px;
                    background: VortexPalette.button-primary;
                    border-radius: 4px;

                    states [
                        hover when retry-touch.has-hover: {
                            background: VortexPalette.button-primary-hover;
                        }
                    ]

                    Text {
                        text: "Retry";
                        color: white;
                        font-size: VortexTypography.font-base;
                        font-weight: VortexTypography.weight-medium;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }

                    retry-touch := TouchArea {
                        clicked => { retry-clicked(); }
                    }
                }
            }
        }
    }

    // Helper function for status color
    pure function status-color() -> color {
        if status-code >= 200 && status-code < 300 { return VortexPalette.status-success; }
        if status-code >= 300 && status-code < 400 { return VortexPalette.status-info; }
        if status-code >= 400 && status-code < 500 { return VortexPalette.status-warning; }
        if status-code >= 500 { return VortexPalette.status-error; }
        return VortexPalette.text-secondary;
    }
}
```

### File: ui/main.slint

```slint
// /vortex/crates/vortex-ui/ui/main.slint

import { VerticalBox, HorizontalBox, TextEdit, Button, ScrollView, LineEdit } from "std-widgets.slint";
import { VortexPalette, VortexTypography, VortexSpacing } from "theme.slint";
import { UrlBar } from "components/url_bar.slint";
import { ResponsePanel } from "components/response_panel.slint";

export component MainWindow inherits Window {
    title: "Vortex API Client";
    min-width: 800px;
    min-height: 600px;
    preferred-width: 1200px;
    preferred-height: 800px;
    background: VortexPalette.bg-primary;

    // Request state
    in-out property <string> url: "";
    in-out property <int> method-index: 0;
    in-out property <string> request-body: "";

    // Response state (0=Idle, 1=Loading, 2=Success, 3=Error)
    in-out property <int> response-state: 0;

    // Response data
    in-out property <int> status-code: 0;
    in-out property <string> status-text: "";
    in-out property <string> response-body: "";
    in-out property <string> duration: "";
    in-out property <string> size: "";

    // Error data
    in-out property <string> error-title: "";
    in-out property <string> error-message: "";
    in-out property <[string]> error-suggestions: [];

    // Loading data
    in-out property <string> elapsed-time: "0ms";

    // Callbacks to Rust
    callback send-request();
    callback cancel-request();
    callback copy-response-body();

    VerticalLayout {
        spacing: 0;

        // Title bar
        Rectangle {
            height: 36px;
            background: VortexPalette.bg-secondary;
            border-width: 0 0 1px 0;
            border-color: VortexPalette.border-default;

            HorizontalLayout {
                padding-left: VortexSpacing.md;
                padding-right: VortexSpacing.md;

                Text {
                    text: "Vortex";
                    color: VortexPalette.text-primary;
                    font-size: VortexTypography.font-lg;
                    font-weight: VortexTypography.weight-bold;
                    vertical-alignment: center;
                }

                Rectangle { horizontal-stretch: 1; }

                Text {
                    text: "v0.1.0";
                    color: VortexPalette.text-secondary;
                    font-size: VortexTypography.font-sm;
                    vertical-alignment: center;
                }
            }
        }

        // Main content area
        HorizontalLayout {
            spacing: 0;

            // Sidebar placeholder (for future collections)
            Rectangle {
                width: 250px;
                background: VortexPalette.bg-secondary;
                border-width: 0 1px 0 0;
                border-color: VortexPalette.border-default;

                VerticalLayout {
                    padding: VortexSpacing.md;
                    spacing: VortexSpacing.sm;

                    Text {
                        text: "COLLECTIONS";
                        color: VortexPalette.text-secondary;
                        font-size: VortexTypography.font-xs;
                        font-weight: VortexTypography.weight-bold;
                        letter-spacing: 0.5px;
                    }

                    Rectangle {
                        vertical-stretch: 1;

                        Text {
                            text: "No collections yet\n\nCollections will be\navailable in Sprint 02";
                            color: VortexPalette.text-secondary;
                            font-size: VortexTypography.font-sm;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }
                    }
                }
            }

            // Main editor area
            VerticalLayout {
                horizontal-stretch: 1;
                spacing: 0;

                // URL Bar
                UrlBar {
                    url <=> root.url;
                    method-index <=> root.method-index;
                    is-loading: response-state == 1;

                    send-clicked => { send-request(); }
                    cancel-clicked => { cancel-request(); }
                }

                // Request body editor (for POST/PUT/PATCH)
                if method-index == 1 || method-index == 2 || method-index == 3: Rectangle {
                    height: 200px;
                    background: VortexPalette.bg-primary;
                    border-width: 0 0 1px 0;
                    border-color: VortexPalette.border-default;

                    VerticalLayout {
                        padding: VortexSpacing.sm;
                        spacing: VortexSpacing.xs;

                        Text {
                            text: "Request Body (JSON)";
                            color: VortexPalette.text-secondary;
                            font-size: VortexTypography.font-sm;
                        }

                        Rectangle {
                            vertical-stretch: 1;
                            background: VortexPalette.bg-input;
                            border-radius: 4px;
                            border-width: 1px;
                            border-color: VortexPalette.border-default;

                            body-editor := TextEdit {
                                text <=> request-body;
                                font-size: VortexTypography.font-base;
                            }
                        }
                    }
                }

                // Response panel
                ResponsePanel {
                    vertical-stretch: 1;
                    state: response-state;
                    status-code: root.status-code;
                    status-text: root.status-text;
                    response-body: root.response-body;
                    duration: root.duration;
                    size: root.size;
                    error-title: root.error-title;
                    error-message: root.error-message;
                    error-suggestions: root.error-suggestions;
                    elapsed-time: root.elapsed-time;

                    retry-clicked => { send-request(); }
                    copy-body-clicked => { copy-response-body(); }
                }
            }
        }
    }
}
```

### File: src/main.rs

```rust
// /vortex/crates/vortex-ui/src/main.rs

//! Vortex API Client - Main Entry Point
//!
//! This is the application entry point that initializes:
//! - The Slint UI
//! - The Tokio async runtime
//! - The HTTP client infrastructure
//! - The UI-to-domain bridge

use std::sync::Arc;

use slint::ComponentHandle;
use tokio::sync::mpsc;
use vortex_application::{CancellationToken, ExecuteRequest, ExecuteResultExt};
use vortex_domain::{HttpMethod, RequestBody, RequestSpec, RequestState};
use vortex_infrastructure::ReqwestHttpClient;

slint::include_modules!();

mod bridge;
use bridge::{UiCommand, UiUpdate};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the Slint UI
    let ui = MainWindow::new()?;
    let ui_weak = ui.as_weak();

    // Create channels for UI <-> async communication
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<UiCommand>();
    let (update_tx, update_rx) = mpsc::unbounded_channel::<UiUpdate>();

    // Store the command sender in the UI callbacks
    let cmd_tx_send = cmd_tx.clone();
    let cmd_tx_cancel = cmd_tx.clone();

    // Set up UI callbacks
    ui.on_send_request(move || {
        let _ = cmd_tx_send.send(UiCommand::SendRequest);
    });

    ui.on_cancel_request(move || {
        let _ = cmd_tx_cancel.send(UiCommand::CancelRequest);
    });

    ui.on_copy_response_body(move || {
        // TODO: Implement clipboard copy
        println!("Copy to clipboard not yet implemented");
    });

    // Spawn the async runtime in a separate thread
    let ui_weak_async = ui_weak.clone();
    std::thread::spawn(move || {
        run_async_runtime(ui_weak_async, cmd_rx, update_tx);
    });

    // Process UI updates on the main thread
    let ui_weak_update = ui_weak.clone();
    slint::Timer::default().start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16), // ~60fps
        move || {
            while let Ok(update) = update_rx.try_recv() {
                if let Some(ui) = ui_weak_update.upgrade() {
                    apply_update(&ui, update);
                }
            }
        },
    );

    // Run the UI event loop
    ui.run()?;

    Ok(())
}

/// Runs the async runtime for handling HTTP requests.
fn run_async_runtime(
    ui_weak: slint::Weak<MainWindow>,
    mut cmd_rx: mpsc::UnboundedReceiver<UiCommand>,
    update_tx: mpsc::UnboundedSender<UiUpdate>,
) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    rt.block_on(async move {
        // Initialize infrastructure
        let http_client = Arc::new(
            ReqwestHttpClient::new().expect("Failed to create HTTP client"),
        );
        let execute_request = ExecuteRequest::new(http_client);

        // Track current cancellation token
        let mut current_cancel: Option<CancellationToken> = None;

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                UiCommand::SendRequest => {
                    // Get current request data from UI
                    let request_data = ui_weak
                        .upgrade_in_event_loop(|ui| {
                            let url = ui.get_url().to_string();
                            let method_index = ui.get_method_index() as usize;
                            let body = ui.get_request_body().to_string();

                            (url, method_index, body)
                        })
                        .ok();

                    if let Some((url, method_index, body)) = request_data {
                        // Create request spec
                        let method = match method_index {
                            0 => HttpMethod::Get,
                            1 => HttpMethod::Post,
                            2 => HttpMethod::Put,
                            3 => HttpMethod::Patch,
                            4 => HttpMethod::Delete,
                            5 => HttpMethod::Head,
                            6 => HttpMethod::Options,
                            _ => HttpMethod::Get,
                        };

                        let request_body = if method.has_body() && !body.is_empty() {
                            RequestBody::json(body)
                        } else {
                            RequestBody::None
                        };

                        let request = RequestSpec {
                            method,
                            url,
                            body: request_body,
                            ..Default::default()
                        };

                        // Update UI to loading state
                        let _ = update_tx.send(UiUpdate::State(RequestState::loading()));

                        // Create cancellation token
                        let (cancel_token, cancel_receiver) = CancellationToken::new();
                        current_cancel = Some(cancel_token);

                        // Execute request with cancellation support
                        let result = execute_request
                            .execute_with_cancellation(&request, cancel_receiver)
                            .await;

                        // Convert result to RequestState
                        let state = result.to_request_state();

                        // Update UI with result
                        let _ = update_tx.send(UiUpdate::State(state));

                        // Clear cancellation token
                        current_cancel = None;
                    }
                }

                UiCommand::CancelRequest => {
                    if let Some(cancel) = current_cancel.take() {
                        cancel.cancel();
                    }
                }
            }
        }
    });
}

/// Applies a UI update to the Slint window.
fn apply_update(ui: &MainWindow, update: UiUpdate) {
    match update {
        UiUpdate::State(state) => {
            match state {
                RequestState::Idle => {
                    ui.set_response_state(0);
                }
                RequestState::Loading { .. } => {
                    ui.set_response_state(1);
                    ui.set_elapsed_time("0ms".into());
                }
                RequestState::Success { response } => {
                    ui.set_response_state(2);
                    ui.set_status_code(response.status.as_u16() as i32);
                    ui.set_status_text(response.status.reason_phrase().into());
                    ui.set_response_body(response.body_as_string_lossy().into());
                    ui.set_duration(response.duration_display().into());
                    ui.set_size(response.size_display().into());
                }
                RequestState::Error {
                    kind,
                    message,
                    details,
                } => {
                    ui.set_response_state(3);
                    ui.set_error_title(kind.title().into());
                    ui.set_error_message(details.unwrap_or(message).into());

                    // Convert suggestions to Slint model
                    let suggestions: Vec<slint::SharedString> = kind
                        .suggestions()
                        .iter()
                        .map(|s| (*s).into())
                        .collect();
                    let model = std::rc::Rc::new(slint::VecModel::from(suggestions));
                    ui.set_error_suggestions(model.into());
                }
            }
        }

        UiUpdate::ElapsedTime(elapsed) => {
            ui.set_elapsed_time(elapsed.into());
        }
    }
}
```

### File: src/bridge.rs

```rust
// /vortex/crates/vortex-ui/src/bridge.rs

//! UI Bridge Module
//!
//! Defines the communication protocol between the Slint UI thread
//! and the async Tokio runtime.

use vortex_domain::RequestState;

/// Commands sent from UI to the async runtime.
#[derive(Debug, Clone)]
pub enum UiCommand {
    /// User clicked Send button or pressed Ctrl+Enter.
    SendRequest,

    /// User clicked Cancel button.
    CancelRequest,
}

/// Updates sent from async runtime to the UI.
#[derive(Debug, Clone)]
pub enum UiUpdate {
    /// Update the request state (Idle/Loading/Success/Error).
    State(RequestState),

    /// Update the elapsed time display during loading.
    ElapsedTime(String),
}
```

---

## Integration: Connecting UI to Domain

The integration follows this flow:

```
┌─────────────┐     UiCommand     ┌─────────────────┐     RequestSpec     ┌─────────────────┐
│   Slint UI  │ ───────────────>  │  Async Runtime  │ ─────────────────>  │  ExecuteRequest │
│  (main.rs)  │                   │   (tokio)       │                     │   (use case)    │
└─────────────┘                   └─────────────────┘                     └─────────────────┘
       ▲                                  │                                       │
       │         UiUpdate                 │                                       │
       └──────────────────────────────────┘                                       │
                                          ▲                                       │
                                          │         ResponseSpec                  │
                                          └───────────────────────────────────────┘
```

### Key Integration Points

1. **Callback Registration** (`main.rs`):
   - `on_send_request` -> sends `UiCommand::SendRequest` to channel
   - `on_cancel_request` -> sends `UiCommand::CancelRequest` to channel

2. **Request Building** (async runtime):
   - Read `url`, `method_index`, `request_body` from UI
   - Construct `RequestSpec` with appropriate `HttpMethod` and `RequestBody`

3. **State Updates** (async runtime to UI):
   - Send `UiUpdate::State(RequestState::loading())` before execution
   - Send `UiUpdate::State(result.to_request_state())` after execution

4. **UI Property Mapping**:

| Domain Type | UI Property | Type |
|-------------|-------------|------|
| `RequestState::Idle` | `response_state = 0` | int |
| `RequestState::Loading` | `response_state = 1` | int |
| `RequestState::Success` | `response_state = 2` | int |
| `RequestState::Error` | `response_state = 3` | int |
| `StatusCode` | `status_code` | i32 |
| `reason_phrase()` | `status_text` | string |
| `body_as_string_lossy()` | `response_body` | string |
| `duration_display()` | `duration` | string |
| `size_display()` | `size` | string |
| `RequestErrorKind::title()` | `error_title` | string |
| `message/details` | `error_message` | string |
| `suggestions()` | `error_suggestions` | [string] |

---

## Implementation Order

This section defines the exact sequence of tasks with dependencies. Each task builds on previous ones.

### Phase 1: Project Setup (Day 1 Morning)

**Task 1.1: Create workspace structure**
```bash
mkdir -p vortex/crates/{vortex-domain,vortex-application,vortex-infrastructure,vortex-ui}
touch vortex/Cargo.toml
touch vortex/rust-toolchain.toml
```

**Task 1.2: Initialize rust-toolchain.toml**
```toml
[toolchain]
channel = "1.93"
```

**Task 1.3: Create workspace Cargo.toml**
- Copy the workspace Cargo.toml from this document

**Task 1.4: Create crate Cargo.toml files**
- `vortex-domain/Cargo.toml`
- `vortex-application/Cargo.toml`
- `vortex-infrastructure/Cargo.toml`
- `vortex-ui/Cargo.toml`

**Verification:** `cargo check` should succeed with empty lib.rs files

### Phase 2: Domain Layer (Day 1 Afternoon)

**Task 2.1: Implement HttpMethod enum**
- File: `vortex-domain/src/request.rs`
- Include: all variants, `as_str()`, `has_body()`, `from_str_case_insensitive()`
- Tests: method display, parsing

**Task 2.2: Implement RequestBody enum**
- File: `vortex-domain/src/request.rs`
- Include: `None`, `Text`, `Json` variants (others as stubs)
- Tests: content_type(), is_none()

**Task 2.3: Implement KeyValuePair struct**
- File: `vortex-domain/src/request.rs`
- Include: constructors, enabled flag

**Task 2.4: Implement RequestSpec struct**
- File: `vortex-domain/src/request.rs`
- Include: builder pattern methods, `full_url()`
- Tests: builder, URL construction

**Task 2.5: Implement StatusCode struct**
- File: `vortex-domain/src/response.rs`
- Include: status categories, reason phrases, color category
- Tests: category detection, display

**Task 2.6: Implement ResponseSpec struct**
- File: `vortex-domain/src/response.rs`
- Include: body conversion methods, display helpers
- Tests: body parsing, size formatting

**Task 2.7: Implement RequestState enum**
- File: `vortex-domain/src/state.rs`
- Include: all states, constructors, accessor methods
- Tests: state transitions

**Task 2.8: Implement RequestErrorKind enum**
- File: `vortex-domain/src/state.rs`
- Include: all variants, suggestions(), title()

**Task 2.9: Create domain lib.rs**
- Export all public types

**Verification:** `cargo test -p vortex-domain` all tests pass

### Phase 3: Application Layer (Day 2 Morning)

**Task 3.1: Define HttpClientError enum**
- File: `vortex-application/src/ports.rs`
- Include: all variants, `to_error_kind()` conversion

**Task 3.2: Define HttpClient trait**
- File: `vortex-application/src/ports.rs`
- Include: `execute()` method signature with Pin<Box<Future>>

**Task 3.3: Implement CancellationToken**
- File: `vortex-application/src/ports.rs`
- Include: sender and receiver types

**Task 3.4: Implement ExecuteRequest use case**
- File: `vortex-application/src/execute_request.rs`
- Include: validation, execution, cancellation support
- Tests: success, empty URL, invalid URL, HTTP error, cancellation

**Task 3.5: Create application lib.rs**
- Export all public types and traits

**Verification:** `cargo test -p vortex-application` all tests pass

### Phase 4: Infrastructure Layer (Day 2 Afternoon)

**Task 4.1: Implement ReqwestHttpClient**
- File: `vortex-infrastructure/src/http_client.rs`
- Include: constructor, method conversion, body building, error mapping

**Task 4.2: Implement HttpClient trait for ReqwestHttpClient**
- Include: full `execute()` implementation with timing

**Task 4.3: Create infrastructure lib.rs**
- Export `ReqwestHttpClient`

**Task 4.4: Integration tests with wiremock**
- Test GET, POST, headers, query params, JSON body validation

**Verification:** `cargo test -p vortex-infrastructure` all tests pass

### Phase 5: UI Layer - Theme and Components (Day 3 Morning)

**Task 5.1: Create theme.slint**
- Define VortexPalette, VortexTypography, VortexSpacing globals

**Task 5.2: Create method_selector.slint**
- Dropdown with colored method names
- Popup for selection

**Task 5.3: Create url_bar.slint**
- Method selector + URL input + Send/Cancel button
- Conditional button based on loading state

**Task 5.4: Create response_panel.slint**
- Four states: Idle, Loading, Success, Error
- Status bar with code, duration, size
- Body display area
- Error display with suggestions

**Verification:** Components render in Slint preview

### Phase 6: UI Layer - Integration (Day 3 Afternoon)

**Task 6.1: Create main.slint**
- 3-column layout (sidebar placeholder + editor + response)
- Wire all properties and callbacks

**Task 6.2: Create bridge.rs**
- Define UiCommand and UiUpdate enums

**Task 6.3: Create build.rs**
- Slint compilation setup

**Task 6.4: Implement main.rs**
- Initialize UI
- Set up channels
- Spawn async runtime
- Register callbacks
- Process updates

**Task 6.5: Implement apply_update function**
- Map RequestState to UI properties
- Handle all state variants

**Verification:** Application launches and shows UI

### Phase 7: End-to-End Testing (Day 4)

**Task 7.1: Manual testing - GET request**
- URL: `https://httpbin.org/get`
- Verify: status 200, response body displayed

**Task 7.2: Manual testing - POST with JSON**
- URL: `https://httpbin.org/post`
- Body: `{"test": true}`
- Verify: status 200, echoed body in response

**Task 7.3: Manual testing - Error handling**
- URL: `https://nonexistent.example.com`
- Verify: Error state shown with suggestions

**Task 7.4: Manual testing - Timeout**
- URL: `https://httpbin.org/delay/10`
- Timeout: 2000ms
- Verify: Timeout error shown

**Task 7.5: Manual testing - Cancel**
- URL: `https://httpbin.org/delay/10`
- Click Cancel during loading
- Verify: Returns to idle or shows cancelled

**Task 7.6: Fix any issues found**

### Phase 8: Polish and Documentation (Day 4-5)

**Task 8.1: Add doc comments to all public items**

**Task 8.2: Run clippy and fix warnings**
```bash
cargo clippy --all-targets -- -D warnings
```

**Task 8.3: Format code**
```bash
cargo fmt --all
```

**Task 8.4: Update README with build instructions**

**Verification:** `cargo build --release` succeeds, binary runs correctly

---

## Testing Strategy

### Unit Tests (Per Crate)

**vortex-domain:**
- HttpMethod parsing and display
- RequestSpec URL building
- StatusCode categorization
- ResponseSpec body conversion
- RequestState transitions

**vortex-application:**
- ExecuteRequest validation
- Result to RequestState conversion
- Mock HTTP client responses

**vortex-infrastructure:**
- ReqwestHttpClient with wiremock
- Error mapping
- Timeout handling

### Integration Tests

```rust
// tests/integration_test.rs
#[tokio::test]
async fn test_full_request_flow() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&mock_server)
        .await;

    let client = Arc::new(ReqwestHttpClient::new().unwrap());
    let use_case = ExecuteRequest::new(client);

    let request = RequestSpec::get(format!("{}/api/test", mock_server.uri()));
    let result = use_case.execute(&request).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.status.as_u16(), 200);
    assert!(response.is_json());
}
```

### Manual Testing Checklist

- [ ] GET request to public API
- [ ] POST request with JSON body
- [ ] Request with custom headers
- [ ] Request with query parameters
- [ ] Invalid URL handling
- [ ] Network error handling
- [ ] Timeout handling
- [ ] Cancel button functionality
- [ ] Method selector changes color
- [ ] Response body scrolling
- [ ] Status code coloring
- [ ] Duration and size display
- [ ] Error suggestions display

---

## Acceptance Criteria

### Functional Requirements

1. **URL Input**: User can enter any URL starting with http:// or https://
2. **Method Selection**: User can select GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS
3. **Send Request**: Clicking Send executes the HTTP request
4. **Response Display**: Status code, body, duration, and size are shown
5. **Error Display**: Network errors show title, message, and suggestions
6. **Loading State**: UI shows loading indicator during request
7. **Cancel Request**: User can cancel in-flight requests

### Non-Functional Requirements

1. **UI Responsiveness**: UI never blocks during HTTP requests
2. **Startup Time**: Application launches in <1 second
3. **Memory Usage**: <100 MB RAM with no requests loaded
4. **Request Overhead**: <50ms added latency from application

### Quality Requirements

1. **Code Compiles**: `cargo build` succeeds without warnings
2. **Tests Pass**: `cargo test` all tests pass
3. **Lint Clean**: `cargo clippy` no warnings
4. **Formatted**: `cargo fmt --check` passes
5. **Documented**: All public APIs have doc comments

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Slint styling limitations | Medium | Use native Slint widgets where possible; custom only when necessary |
| Large response bodies | High | Implement virtual scrolling in Sprint 02; truncate display for MVP |
| URL parsing edge cases | Medium | Use robust URL parsing; show clear error messages |
| Cross-platform issues | Medium | Test on macOS, Windows, Linux early |
| Async/UI thread sync | High | Use proper channel communication; never block UI thread |

---

## Related Documents

- [Product Vision](./00-product-vision.md)
- [File Format Specification](./02-file-format-spec.md)
- [UI/UX Specification](./03-ui-ux-specification.md)

---

## Sprint 02 Preview

The next sprint will add:
- Local file persistence (save/load requests)
- Collections sidebar with tree view
- Request tabs for multiple requests
- Basic variable support (`{{variable}}` syntax)
- Environment switching
