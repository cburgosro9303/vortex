# Sprint 07 — Extensibility (Protocols & Plugins)

**Objective:** Design and implement a plugin architecture that enables protocol extensibility, custom authentication methods, and import/export formats while maintaining security through sandboxing.

**Duration:** 2 weeks

---

## Scope

### In Scope
- Plugin manifest format and loading mechanism
- Protocol abstraction layer (trait-based)
- Plugin registry and lifecycle management
- HTTP as default protocol implementation
- Mock protocol plugin as reference example
- UI components for plugin management
- Security sandbox design

### Out of Scope
- Full gRPC/WebSocket implementation (stubs only)
- Remote plugin marketplace
- Plugin hot-reloading
- Plugin debugging tools

---

## 1. Plugin Architecture

### 1.1 Plugin Manifest Format

Each plugin must include a `plugin.json` manifest file in its root directory.

```json
{
  "manifest_version": 1,
  "id": "com.example.mock-protocol",
  "name": "Mock Protocol",
  "version": "1.0.0",
  "description": "A mock protocol for testing purposes",
  "authors": ["Developer Name <dev@example.com>"],
  "license": "MIT",
  "homepage": "https://github.com/example/vortex-mock-plugin",
  "repository": "https://github.com/example/vortex-mock-plugin",
  "minimum_vortex_version": "1.0.0",
  "capabilities": ["protocol"],
  "protocol": {
    "scheme": "mock",
    "display_name": "Mock Protocol",
    "default_port": null
  },
  "permissions": {
    "network": false,
    "filesystem": {
      "read": [],
      "write": []
    },
    "environment_variables": false,
    "secrets_access": false
  },
  "entry_point": "plugin.wasm",
  "config_schema": {
    "type": "object",
    "properties": {
      "default_delay_ms": {
        "type": "integer",
        "default": 0,
        "description": "Default response delay in milliseconds"
      }
    }
  }
}
```

### 1.2 Manifest Schema Definition

```rust
// crates/domain/src/plugin/manifest.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin manifest version for schema evolution
pub const MANIFEST_VERSION: u32 = 1;

/// Root manifest structure loaded from plugin.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub manifest_version: u32,
    pub id: PluginId,
    pub name: String,
    pub version: semver::Version,
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    pub minimum_vortex_version: semver::Version,
    pub capabilities: Vec<PluginCapability>,
    #[serde(default)]
    pub protocol: Option<ProtocolDefinition>,
    #[serde(default)]
    pub auth: Option<AuthDefinition>,
    #[serde(default)]
    pub importer: Option<ImporterDefinition>,
    #[serde(default)]
    pub exporter: Option<ExporterDefinition>,
    pub permissions: PluginPermissions,
    pub entry_point: String,
    #[serde(default)]
    pub config_schema: Option<serde_json::Value>,
}

/// Unique plugin identifier (reverse domain notation)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginId(pub String);

impl PluginId {
    pub fn new(id: impl Into<String>) -> Result<Self, PluginError> {
        let id = id.into();
        // Validate reverse domain notation
        if !id.contains('.') || id.starts_with('.') || id.ends_with('.') {
            return Err(PluginError::InvalidId(id));
        }
        Ok(Self(id))
    }
}

/// Capabilities that a plugin can provide
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    /// Adds a new protocol (HTTP, gRPC, WebSocket, custom)
    Protocol,
    /// Adds authentication method (OAuth, custom)
    Auth,
    /// Imports collections from external formats
    Importer,
    /// Exports collections to external formats
    Exporter,
}

/// Protocol-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolDefinition {
    pub scheme: String,
    pub display_name: String,
    #[serde(default)]
    pub default_port: Option<u16>,
    #[serde(default)]
    pub supports_streaming: bool,
}

/// Auth provider metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthDefinition {
    pub auth_type: String,
    pub display_name: String,
    #[serde(default)]
    pub config_fields: Vec<ConfigField>,
}

/// Importer metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImporterDefinition {
    pub format_name: String,
    pub file_extensions: Vec<String>,
    pub mime_types: Vec<String>,
}

/// Exporter metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExporterDefinition {
    pub format_name: String,
    pub file_extension: String,
    pub mime_type: String,
}

/// Configuration field descriptor for UI generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    pub name: String,
    pub display_name: String,
    pub field_type: ConfigFieldType,
    pub required: bool,
    #[serde(default)]
    pub default_value: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigFieldType {
    String,
    Secret,
    Integer,
    Boolean,
    Enum { options: Vec<String> },
}

/// Permissions requested by the plugin
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginPermissions {
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub filesystem: FilesystemPermissions,
    #[serde(default)]
    pub environment_variables: bool,
    #[serde(default)]
    pub secrets_access: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilesystemPermissions {
    #[serde(default)]
    pub read: Vec<String>,
    #[serde(default)]
    pub write: Vec<String>,
}

/// Plugin-related errors
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Invalid plugin ID: {0}")]
    InvalidId(String),
    #[error("Manifest parsing failed: {0}")]
    ManifestParse(#[from] serde_json::Error),
    #[error("Unsupported manifest version: {0}")]
    UnsupportedVersion(u32),
    #[error("Missing required capability: {0:?}")]
    MissingCapability(PluginCapability),
    #[error("Plugin not found: {0}")]
    NotFound(String),
    #[error("Plugin load failed: {0}")]
    LoadFailed(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Plugin execution error: {0}")]
    ExecutionError(String),
}
```

---

## 2. Protocol Abstraction Layer

### 2.1 Core Protocol Traits

```rust
// crates/domain/src/protocol/mod.rs

use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

/// Unique identifier for protocol handlers
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolScheme(pub String);

impl ProtocolScheme {
    pub const HTTP: ProtocolScheme = ProtocolScheme(String::new()); // "http"
    pub const HTTPS: ProtocolScheme = ProtocolScheme(String::new()); // "https"
    pub const GRPC: ProtocolScheme = ProtocolScheme(String::new()); // "grpc"
    pub const WS: ProtocolScheme = ProtocolScheme(String::new()); // "ws"
    pub const WSS: ProtocolScheme = ProtocolScheme(String::new()); // "wss"

    pub fn new(scheme: impl Into<String>) -> Self {
        Self(scheme.into().to_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Protocol-agnostic request representation
#[derive(Debug, Clone)]
pub struct ProtocolRequest {
    /// Target URL/URI
    pub url: String,
    /// Protocol-specific method (GET, POST for HTTP; unary/stream for gRPC)
    pub method: String,
    /// Headers/metadata
    pub headers: HashMap<String, String>,
    /// Request body
    pub body: RequestBody,
    /// Request-specific options
    pub options: RequestOptions,
}

/// Request body variants
#[derive(Debug, Clone)]
pub enum RequestBody {
    None,
    Text(String),
    Binary(Vec<u8>),
    Json(serde_json::Value),
    FormData(Vec<FormField>),
    FormUrlEncoded(HashMap<String, String>),
    /// For gRPC: protobuf message
    Protobuf { message_type: String, data: Vec<u8> },
}

#[derive(Debug, Clone)]
pub struct FormField {
    pub name: String,
    pub value: FormFieldValue,
}

#[derive(Debug, Clone)]
pub enum FormFieldValue {
    Text(String),
    File { filename: String, content: Vec<u8>, content_type: String },
}

/// Request execution options
#[derive(Debug, Clone)]
pub struct RequestOptions {
    pub timeout: Duration,
    pub follow_redirects: bool,
    pub max_redirects: u32,
    pub verify_ssl: bool,
    #[serde(default)]
    pub client_cert: Option<ClientCertificate>,
    /// Protocol-specific options as JSON
    pub extra: HashMap<String, serde_json::Value>,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            follow_redirects: true,
            max_redirects: 10,
            verify_ssl: true,
            client_cert: None,
            extra: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClientCertificate {
    pub cert_pem: String,
    pub key_pem: String,
    pub passphrase: Option<String>,
}

/// Protocol-agnostic response representation
#[derive(Debug, Clone)]
pub struct ProtocolResponse {
    /// Status code (HTTP status, gRPC status code)
    pub status: ResponseStatus,
    /// Response headers/metadata
    pub headers: HashMap<String, String>,
    /// Response body
    pub body: ResponseBody,
    /// Response timing metrics
    pub timing: ResponseTiming,
    /// Protocol-specific metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ResponseStatus {
    pub code: u32,
    pub message: String,
}

impl ResponseStatus {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.code)
    }

    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.code)
    }

    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.code)
    }
}

#[derive(Debug, Clone)]
pub enum ResponseBody {
    None,
    Text(String),
    Binary(Vec<u8>),
    /// Streaming response (for SSE, WebSocket, gRPC streams)
    Stream(StreamHandle),
}

/// Handle to a streaming response
#[derive(Debug, Clone)]
pub struct StreamHandle {
    pub id: String,
    pub protocol: String,
}

#[derive(Debug, Clone, Default)]
pub struct ResponseTiming {
    pub dns_lookup: Option<Duration>,
    pub tcp_connect: Option<Duration>,
    pub tls_handshake: Option<Duration>,
    pub time_to_first_byte: Duration,
    pub total_time: Duration,
    pub download_size: u64,
}

/// Core trait that all protocol handlers must implement
#[async_trait]
pub trait ProtocolHandler: Send + Sync {
    /// Returns the protocol scheme(s) this handler supports
    fn schemes(&self) -> Vec<ProtocolScheme>;

    /// Human-readable name for UI display
    fn display_name(&self) -> &str;

    /// Execute a request and return the response
    async fn execute(
        &self,
        request: ProtocolRequest,
        context: &ExecutionContext,
    ) -> Result<ProtocolResponse, ProtocolError>;

    /// Validate a request before execution
    fn validate(&self, request: &ProtocolRequest) -> Result<(), ProtocolError> {
        // Default: no validation
        Ok(())
    }

    /// Check if this handler supports streaming
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Cancel an ongoing request (for streaming protocols)
    async fn cancel(&self, _request_id: &str) -> Result<(), ProtocolError> {
        Ok(())
    }
}

/// Context provided during request execution
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub request_id: String,
    pub variables: HashMap<String, String>,
    pub secrets: HashMap<String, String>,
}

/// Protocol execution errors
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Request timeout after {0:?}")]
    Timeout(Duration),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("SSL/TLS error: {0}")]
    TlsError(String),
    #[error("DNS resolution failed: {0}")]
    DnsError(String),
    #[error("Request cancelled")]
    Cancelled,
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Protocol error: {0}")]
    ProtocolSpecific(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

### 2.2 Protocol Registry

```rust
// crates/application/src/protocol/registry.rs

use crate::domain::protocol::{ProtocolHandler, ProtocolScheme, ProtocolError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Registry for protocol handlers
pub struct ProtocolRegistry {
    handlers: RwLock<HashMap<ProtocolScheme, Arc<dyn ProtocolHandler>>>,
    default_scheme: RwLock<Option<ProtocolScheme>>,
}

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            default_scheme: RwLock::new(None),
        }
    }

    /// Create registry with HTTP as default handler
    pub async fn with_http_default() -> Self {
        let registry = Self::new();
        let http_handler = Arc::new(super::http::HttpProtocolHandler::new());
        registry.register(http_handler).await.expect("HTTP registration");
        registry.set_default(ProtocolScheme::new("https")).await;
        registry
    }

    /// Register a protocol handler
    pub async fn register(
        &self,
        handler: Arc<dyn ProtocolHandler>,
    ) -> Result<(), RegistryError> {
        let schemes = handler.schemes();
        if schemes.is_empty() {
            return Err(RegistryError::NoSchemes);
        }

        let mut handlers = self.handlers.write().await;
        for scheme in schemes {
            if handlers.contains_key(&scheme) {
                return Err(RegistryError::SchemeAlreadyRegistered(scheme.0.clone()));
            }
            handlers.insert(scheme, Arc::clone(&handler));
        }
        Ok(())
    }

    /// Unregister a protocol handler by scheme
    pub async fn unregister(&self, scheme: &ProtocolScheme) -> Result<(), RegistryError> {
        let mut handlers = self.handlers.write().await;
        handlers.remove(scheme)
            .ok_or_else(|| RegistryError::SchemeNotFound(scheme.0.clone()))?;
        Ok(())
    }

    /// Get handler for a specific scheme
    pub async fn get(&self, scheme: &ProtocolScheme) -> Option<Arc<dyn ProtocolHandler>> {
        self.handlers.read().await.get(scheme).cloned()
    }

    /// Get handler for URL (extracts scheme)
    pub async fn get_for_url(&self, url: &str) -> Result<Arc<dyn ProtocolHandler>, RegistryError> {
        let scheme = extract_scheme(url)
            .ok_or_else(|| RegistryError::InvalidUrl(url.to_string()))?;

        self.handlers.read().await
            .get(&scheme)
            .cloned()
            .ok_or_else(|| RegistryError::SchemeNotFound(scheme.0))
    }

    /// Set the default protocol scheme
    pub async fn set_default(&self, scheme: ProtocolScheme) {
        *self.default_scheme.write().await = Some(scheme);
    }

    /// List all registered schemes
    pub async fn list_schemes(&self) -> Vec<ProtocolScheme> {
        self.handlers.read().await.keys().cloned().collect()
    }

    /// List all handlers with metadata
    pub async fn list_handlers(&self) -> Vec<ProtocolInfo> {
        let handlers = self.handlers.read().await;
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for (scheme, handler) in handlers.iter() {
            let ptr = Arc::as_ptr(handler) as usize;
            if seen.insert(ptr) {
                result.push(ProtocolInfo {
                    schemes: handler.schemes(),
                    display_name: handler.display_name().to_string(),
                    supports_streaming: handler.supports_streaming(),
                });
            }
        }
        result
    }
}

fn extract_scheme(url: &str) -> Option<ProtocolScheme> {
    url.split("://")
        .next()
        .filter(|s| !s.is_empty() && !s.contains('/'))
        .map(ProtocolScheme::new)
}

#[derive(Debug, Clone)]
pub struct ProtocolInfo {
    pub schemes: Vec<ProtocolScheme>,
    pub display_name: String,
    pub supports_streaming: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Handler provides no schemes")]
    NoSchemes,
    #[error("Scheme already registered: {0}")]
    SchemeAlreadyRegistered(String),
    #[error("Scheme not found: {0}")]
    SchemeNotFound(String),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}
```

---

## 3. HTTP Protocol Implementation (Default)

```rust
// crates/infrastructure/src/protocol/http.rs

use async_trait::async_trait;
use domain::protocol::*;
use reqwest::{Client, Method, header::{HeaderMap, HeaderName, HeaderValue}};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{Duration, Instant};

pub struct HttpProtocolHandler {
    client: Client,
}

impl HttpProtocolHandler {
    pub fn new() -> Self {
        Self::with_config(HttpConfig::default())
    }

    pub fn with_config(config: HttpConfig) -> Self {
        let client = Client::builder()
            .timeout(config.default_timeout)
            .pool_max_idle_per_host(config.pool_size)
            .build()
            .expect("Failed to build HTTP client");

        Self { client }
    }
}

#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub default_timeout: Duration,
    pub pool_size: usize,
    pub user_agent: String,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            pool_size: 10,
            user_agent: format!("Vortex/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

#[async_trait]
impl ProtocolHandler for HttpProtocolHandler {
    fn schemes(&self) -> Vec<ProtocolScheme> {
        vec![
            ProtocolScheme::new("http"),
            ProtocolScheme::new("https"),
        ]
    }

    fn display_name(&self) -> &str {
        "HTTP/HTTPS"
    }

    async fn execute(
        &self,
        request: ProtocolRequest,
        context: &ExecutionContext,
    ) -> Result<ProtocolResponse, ProtocolError> {
        let start = Instant::now();

        // Build request
        let method = Method::from_str(&request.method)
            .map_err(|_| ProtocolError::InvalidRequest(
                format!("Invalid HTTP method: {}", request.method)
            ))?;

        let mut req_builder = self.client.request(method, &request.url);

        // Add headers
        let mut headers = HeaderMap::new();
        for (key, value) in &request.headers {
            let name = HeaderName::from_str(key)
                .map_err(|e| ProtocolError::InvalidRequest(format!("Invalid header name: {}", e)))?;
            let val = HeaderValue::from_str(value)
                .map_err(|e| ProtocolError::InvalidRequest(format!("Invalid header value: {}", e)))?;
            headers.insert(name, val);
        }
        req_builder = req_builder.headers(headers);

        // Add body
        req_builder = match request.body {
            RequestBody::None => req_builder,
            RequestBody::Text(text) => req_builder.body(text),
            RequestBody::Binary(bytes) => req_builder.body(bytes),
            RequestBody::Json(json) => req_builder.json(&json),
            RequestBody::FormUrlEncoded(params) => req_builder.form(&params),
            RequestBody::FormData(fields) => {
                let mut form = reqwest::multipart::Form::new();
                for field in fields {
                    form = match field.value {
                        FormFieldValue::Text(text) => form.text(field.name, text),
                        FormFieldValue::File { filename, content, content_type } => {
                            let part = reqwest::multipart::Part::bytes(content)
                                .file_name(filename)
                                .mime_str(&content_type)
                                .map_err(|e| ProtocolError::InvalidRequest(e.to_string()))?;
                            form.part(field.name, part)
                        }
                    };
                }
                req_builder.multipart(form)
            }
            RequestBody::Protobuf { .. } => {
                return Err(ProtocolError::InvalidRequest(
                    "Protobuf not supported for HTTP".to_string()
                ));
            }
        };

        // Apply options
        if !request.options.verify_ssl {
            // Note: Requires rebuilding client for SSL settings
            // This is a simplified version
        }
        req_builder = req_builder.timeout(request.options.timeout);

        // Execute
        let time_to_first_byte_start = Instant::now();
        let response = req_builder.send().await.map_err(|e| {
            if e.is_timeout() {
                ProtocolError::Timeout(request.options.timeout)
            } else if e.is_connect() {
                ProtocolError::ConnectionFailed(e.to_string())
            } else {
                ProtocolError::ProtocolSpecific(e.to_string())
            }
        })?;
        let time_to_first_byte = time_to_first_byte_start.elapsed();

        // Parse response
        let status = ResponseStatus {
            code: response.status().as_u16() as u32,
            message: response.status().canonical_reason()
                .unwrap_or("Unknown")
                .to_string(),
        };

        let mut resp_headers = HashMap::new();
        for (name, value) in response.headers() {
            if let Ok(v) = value.to_str() {
                resp_headers.insert(name.to_string(), v.to_string());
            }
        }

        let bytes = response.bytes().await
            .map_err(|e| ProtocolError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string()
            )))?;
        let download_size = bytes.len() as u64;

        let body = match String::from_utf8(bytes.to_vec()) {
            Ok(text) => ResponseBody::Text(text),
            Err(_) => ResponseBody::Binary(bytes.to_vec()),
        };

        let total_time = start.elapsed();

        Ok(ProtocolResponse {
            status,
            headers: resp_headers,
            body,
            timing: ResponseTiming {
                dns_lookup: None, // Would need custom resolver
                tcp_connect: None,
                tls_handshake: None,
                time_to_first_byte,
                total_time,
                download_size,
            },
            metadata: HashMap::new(),
        })
    }

    fn validate(&self, request: &ProtocolRequest) -> Result<(), ProtocolError> {
        // Validate URL
        if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
            return Err(ProtocolError::InvalidUrl(
                "URL must start with http:// or https://".to_string()
            ));
        }

        // Validate method
        if Method::from_str(&request.method).is_err() {
            return Err(ProtocolError::InvalidRequest(
                format!("Invalid HTTP method: {}", request.method)
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_get_request() {
        let handler = HttpProtocolHandler::new();
        let request = ProtocolRequest {
            url: "https://httpbin.org/get".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            body: RequestBody::None,
            options: RequestOptions::default(),
        };
        let context = ExecutionContext {
            request_id: "test-1".to_string(),
            variables: HashMap::new(),
            secrets: HashMap::new(),
        };

        let response = handler.execute(request, &context).await;
        assert!(response.is_ok());
        let resp = response.unwrap();
        assert!(resp.status.is_success());
    }

    #[test]
    fn test_validate_invalid_url() {
        let handler = HttpProtocolHandler::new();
        let request = ProtocolRequest {
            url: "ftp://example.com".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            body: RequestBody::None,
            options: RequestOptions::default(),
        };

        assert!(handler.validate(&request).is_err());
    }
}
```

---

## 4. Plugin Loader and Manager

```rust
// crates/infrastructure/src/plugin/loader.rs

use domain::plugin::{PluginManifest, PluginId, PluginCapability, PluginError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;

/// Discovers and loads plugins from the filesystem
pub struct PluginLoader {
    plugin_dirs: Vec<PathBuf>,
}

impl PluginLoader {
    pub fn new(plugin_dirs: Vec<PathBuf>) -> Self {
        Self { plugin_dirs }
    }

    /// Default plugin directories for the current platform
    pub fn default_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // User plugins directory
        if let Some(data_dir) = dirs::data_dir() {
            dirs.push(data_dir.join("vortex").join("plugins"));
        }

        // Application plugins directory (bundled)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                dirs.push(exe_dir.join("plugins"));
            }
        }

        dirs
    }

    /// Discover all plugins in configured directories
    pub async fn discover(&self) -> Result<Vec<DiscoveredPlugin>, PluginError> {
        let mut plugins = Vec::new();

        for dir in &self.plugin_dirs {
            if !dir.exists() {
                continue;
            }

            let mut entries = fs::read_dir(dir).await
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?;

            while let Some(entry) = entries.next_entry().await
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?
            {
                let path = entry.path();
                if path.is_dir() {
                    match self.load_manifest(&path).await {
                        Ok(manifest) => {
                            plugins.push(DiscoveredPlugin {
                                path: path.clone(),
                                manifest,
                            });
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load plugin from {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        Ok(plugins)
    }

    /// Load manifest from a plugin directory
    async fn load_manifest(&self, plugin_dir: &Path) -> Result<PluginManifest, PluginError> {
        let manifest_path = plugin_dir.join("plugin.json");

        let content = fs::read_to_string(&manifest_path).await
            .map_err(|e| PluginError::LoadFailed(
                format!("Cannot read manifest: {}", e)
            ))?;

        let manifest: PluginManifest = serde_json::from_str(&content)?;

        // Validate manifest version
        if manifest.manifest_version > domain::plugin::MANIFEST_VERSION {
            return Err(PluginError::UnsupportedVersion(manifest.manifest_version));
        }

        // Validate entry point exists
        let entry_point = plugin_dir.join(&manifest.entry_point);
        if !entry_point.exists() {
            return Err(PluginError::LoadFailed(
                format!("Entry point not found: {:?}", entry_point)
            ));
        }

        Ok(manifest)
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub path: PathBuf,
    pub manifest: PluginManifest,
}

/// Manages plugin lifecycle (load, enable, disable, unload)
pub struct PluginManager {
    loader: PluginLoader,
    plugins: RwLock<HashMap<PluginId, LoadedPlugin>>,
    sandbox: Arc<dyn PluginSandbox>,
}

/// A plugin that has been loaded into memory
#[derive(Debug)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub state: PluginState,
    pub instance: Option<PluginInstance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Discovered,
    Loaded,
    Enabled,
    Disabled,
    Error,
}

/// Plugin instance (WASM module or native)
#[derive(Debug)]
pub enum PluginInstance {
    Wasm(WasmPluginInstance),
    // Future: Native(NativePluginInstance),
}

#[derive(Debug)]
pub struct WasmPluginInstance {
    // wasmtime::Instance or similar
    pub module_path: PathBuf,
}

impl PluginManager {
    pub fn new(loader: PluginLoader, sandbox: Arc<dyn PluginSandbox>) -> Self {
        Self {
            loader,
            plugins: RwLock::new(HashMap::new()),
            sandbox,
        }
    }

    /// Discover and register all available plugins
    pub async fn discover_all(&self) -> Result<Vec<PluginId>, PluginError> {
        let discovered = self.loader.discover().await?;
        let mut plugins = self.plugins.write().await;
        let mut ids = Vec::new();

        for plugin in discovered {
            let id = plugin.manifest.id.clone();
            plugins.insert(id.clone(), LoadedPlugin {
                manifest: plugin.manifest,
                path: plugin.path,
                state: PluginState::Discovered,
                instance: None,
            });
            ids.push(id);
        }

        Ok(ids)
    }

    /// Load a specific plugin
    pub async fn load(&self, id: &PluginId) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;
        let plugin = plugins.get_mut(id)
            .ok_or_else(|| PluginError::NotFound(id.0.clone()))?;

        // Verify permissions
        self.sandbox.verify_permissions(&plugin.manifest.permissions)?;

        // Load WASM module
        let entry_point = plugin.path.join(&plugin.manifest.entry_point);
        plugin.instance = Some(PluginInstance::Wasm(WasmPluginInstance {
            module_path: entry_point,
        }));
        plugin.state = PluginState::Loaded;

        Ok(())
    }

    /// Enable a loaded plugin
    pub async fn enable(&self, id: &PluginId) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;
        let plugin = plugins.get_mut(id)
            .ok_or_else(|| PluginError::NotFound(id.0.clone()))?;

        if plugin.state != PluginState::Loaded && plugin.state != PluginState::Disabled {
            return Err(PluginError::LoadFailed(
                "Plugin must be loaded before enabling".to_string()
            ));
        }

        plugin.state = PluginState::Enabled;
        Ok(())
    }

    /// Disable an enabled plugin
    pub async fn disable(&self, id: &PluginId) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;
        let plugin = plugins.get_mut(id)
            .ok_or_else(|| PluginError::NotFound(id.0.clone()))?;

        plugin.state = PluginState::Disabled;
        Ok(())
    }

    /// Get all plugins with a specific capability
    pub async fn get_by_capability(&self, capability: PluginCapability) -> Vec<PluginId> {
        self.plugins.read().await
            .iter()
            .filter(|(_, p)| {
                p.state == PluginState::Enabled &&
                p.manifest.capabilities.contains(&capability)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// List all plugins
    pub async fn list(&self) -> Vec<PluginInfo> {
        self.plugins.read().await
            .iter()
            .map(|(id, plugin)| PluginInfo {
                id: id.clone(),
                name: plugin.manifest.name.clone(),
                version: plugin.manifest.version.to_string(),
                description: plugin.manifest.description.clone(),
                capabilities: plugin.manifest.capabilities.clone(),
                state: plugin.state,
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<PluginCapability>,
    pub state: PluginState,
}

/// Sandbox interface for plugin execution
pub trait PluginSandbox: Send + Sync {
    fn verify_permissions(&self, permissions: &domain::plugin::PluginPermissions)
        -> Result<(), PluginError>;
    fn create_runtime(&self, manifest: &PluginManifest) -> Result<Box<dyn PluginRuntime>, PluginError>;
}

/// Runtime for executing plugin code
pub trait PluginRuntime: Send + Sync {
    fn call_function(&self, name: &str, args: &[u8]) -> Result<Vec<u8>, PluginError>;
}
```

---

## 5. Mock Protocol Plugin (Example)

### 5.1 Plugin Directory Structure

```
plugins/
  mock-protocol/
    plugin.json
    plugin.wasm
    README.md
```

### 5.2 Plugin Manifest

```json
{
  "manifest_version": 1,
  "id": "com.vortex.mock-protocol",
  "name": "Mock Protocol",
  "version": "1.0.0",
  "description": "Returns configurable static responses for testing",
  "authors": ["Vortex Team"],
  "license": "MIT",
  "minimum_vortex_version": "1.0.0",
  "capabilities": ["protocol"],
  "protocol": {
    "scheme": "mock",
    "display_name": "Mock Protocol",
    "default_port": null,
    "supports_streaming": false
  },
  "permissions": {
    "network": false,
    "filesystem": {
      "read": [],
      "write": []
    },
    "environment_variables": false,
    "secrets_access": false
  },
  "entry_point": "plugin.wasm",
  "config_schema": {
    "type": "object",
    "properties": {
      "default_status": {
        "type": "integer",
        "default": 200
      },
      "default_delay_ms": {
        "type": "integer",
        "default": 0
      },
      "default_body": {
        "type": "string",
        "default": "{\"message\": \"Mock response\"}"
      }
    }
  }
}
```

### 5.3 Native Rust Implementation (for embedded/bundled version)

```rust
// crates/infrastructure/src/protocol/mock.rs

use async_trait::async_trait;
use domain::protocol::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

/// Mock protocol handler for testing
pub struct MockProtocolHandler {
    config: MockConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockConfig {
    #[serde(default = "default_status")]
    pub default_status: u32,
    #[serde(default)]
    pub default_delay_ms: u64,
    #[serde(default = "default_body")]
    pub default_body: String,
    #[serde(default)]
    pub routes: HashMap<String, MockRoute>,
}

fn default_status() -> u32 { 200 }
fn default_body() -> String { r#"{"message": "Mock response"}"#.to_string() }

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            default_status: 200,
            default_delay_ms: 0,
            default_body: default_body(),
            routes: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockRoute {
    pub status: u32,
    pub headers: HashMap<String, String>,
    pub body: String,
    #[serde(default)]
    pub delay_ms: u64,
}

impl MockProtocolHandler {
    pub fn new() -> Self {
        Self::with_config(MockConfig::default())
    }

    pub fn with_config(config: MockConfig) -> Self {
        Self { config }
    }

    /// Parse mock:// URL to extract route key
    fn parse_mock_url(url: &str) -> Option<String> {
        url.strip_prefix("mock://").map(|s| s.to_string())
    }
}

#[async_trait]
impl ProtocolHandler for MockProtocolHandler {
    fn schemes(&self) -> Vec<ProtocolScheme> {
        vec![ProtocolScheme::new("mock")]
    }

    fn display_name(&self) -> &str {
        "Mock Protocol"
    }

    async fn execute(
        &self,
        request: ProtocolRequest,
        _context: &ExecutionContext,
    ) -> Result<ProtocolResponse, ProtocolError> {
        let route_key = Self::parse_mock_url(&request.url)
            .ok_or_else(|| ProtocolError::InvalidUrl(request.url.clone()))?;

        // Find matching route or use defaults
        let (status, headers, body, delay) = if let Some(route) = self.config.routes.get(&route_key) {
            (
                route.status,
                route.headers.clone(),
                route.body.clone(),
                route.delay_ms,
            )
        } else {
            (
                self.config.default_status,
                HashMap::new(),
                self.config.default_body.clone(),
                self.config.default_delay_ms,
            )
        };

        // Simulate delay
        if delay > 0 {
            sleep(Duration::from_millis(delay)).await;
        }

        Ok(ProtocolResponse {
            status: ResponseStatus {
                code: status,
                message: status_message(status).to_string(),
            },
            headers,
            body: ResponseBody::Text(body),
            timing: ResponseTiming {
                dns_lookup: Some(Duration::ZERO),
                tcp_connect: Some(Duration::ZERO),
                tls_handshake: None,
                time_to_first_byte: Duration::from_millis(delay),
                total_time: Duration::from_millis(delay),
                download_size: 0,
            },
            metadata: {
                let mut m = HashMap::new();
                m.insert("mock".to_string(), serde_json::json!(true));
                m.insert("route_key".to_string(), serde_json::json!(route_key));
                m
            },
        })
    }

    fn validate(&self, request: &ProtocolRequest) -> Result<(), ProtocolError> {
        if !request.url.starts_with("mock://") {
            return Err(ProtocolError::InvalidUrl(
                "Mock URL must start with mock://".to_string()
            ));
        }
        Ok(())
    }
}

fn status_message(code: u32) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_default_response() {
        let handler = MockProtocolHandler::new();
        let request = ProtocolRequest {
            url: "mock://test/endpoint".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            body: RequestBody::None,
            options: RequestOptions::default(),
        };
        let context = ExecutionContext {
            request_id: "test".to_string(),
            variables: HashMap::new(),
            secrets: HashMap::new(),
        };

        let response = handler.execute(request, &context).await.unwrap();
        assert_eq!(response.status.code, 200);
    }

    #[tokio::test]
    async fn test_mock_custom_route() {
        let mut routes = HashMap::new();
        routes.insert("users".to_string(), MockRoute {
            status: 201,
            headers: HashMap::new(),
            body: r#"{"id": 1}"#.to_string(),
            delay_ms: 0,
        });

        let handler = MockProtocolHandler::with_config(MockConfig {
            routes,
            ..Default::default()
        });

        let request = ProtocolRequest {
            url: "mock://users".to_string(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body: RequestBody::None,
            options: RequestOptions::default(),
        };
        let context = ExecutionContext {
            request_id: "test".to_string(),
            variables: HashMap::new(),
            secrets: HashMap::new(),
        };

        let response = handler.execute(request, &context).await.unwrap();
        assert_eq!(response.status.code, 201);
    }
}
```

---

## 6. Future Protocol Stubs

### 6.1 gRPC Handler Interface

```rust
// crates/domain/src/protocol/grpc.rs

use super::*;
use async_trait::async_trait;

/// gRPC-specific request extensions
#[derive(Debug, Clone)]
pub struct GrpcRequestExt {
    /// Service name (e.g., "helloworld.Greeter")
    pub service: String,
    /// Method name (e.g., "SayHello")
    pub method: String,
    /// Proto file content or path
    pub proto_source: ProtoSource,
    /// Request message as JSON (will be converted to protobuf)
    pub message_json: serde_json::Value,
    /// Metadata (equivalent to HTTP headers)
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum ProtoSource {
    /// Inline proto definition
    Inline(String),
    /// Path to .proto file
    File(PathBuf),
    /// Server reflection (auto-discover)
    Reflection,
}

/// gRPC-specific response extensions
#[derive(Debug, Clone)]
pub struct GrpcResponseExt {
    /// gRPC status code (0 = OK)
    pub grpc_status: i32,
    /// gRPC status message
    pub grpc_message: String,
    /// Response message as JSON
    pub message_json: serde_json::Value,
    /// Trailing metadata
    pub trailing_metadata: HashMap<String, String>,
}

/// Stub for future gRPC implementation
pub struct GrpcProtocolHandler {
    // Future: tonic client, proto registry
}

#[async_trait]
impl ProtocolHandler for GrpcProtocolHandler {
    fn schemes(&self) -> Vec<ProtocolScheme> {
        vec![ProtocolScheme::new("grpc"), ProtocolScheme::new("grpcs")]
    }

    fn display_name(&self) -> &str {
        "gRPC"
    }

    async fn execute(
        &self,
        _request: ProtocolRequest,
        _context: &ExecutionContext,
    ) -> Result<ProtocolResponse, ProtocolError> {
        Err(ProtocolError::ProtocolSpecific(
            "gRPC support not yet implemented".to_string()
        ))
    }

    fn supports_streaming(&self) -> bool {
        true // gRPC supports bidirectional streaming
    }
}
```

### 6.2 WebSocket Handler Interface

```rust
// crates/domain/src/protocol/websocket.rs

use super::*;
use async_trait::async_trait;

/// WebSocket connection states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketState {
    Connecting,
    Open,
    Closing,
    Closed,
}

/// WebSocket message types
#[derive(Debug, Clone)]
pub enum WebSocketMessage {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close { code: u16, reason: String },
}

/// WebSocket-specific request (connection parameters)
#[derive(Debug, Clone)]
pub struct WebSocketConnectRequest {
    pub url: String,
    pub protocols: Vec<String>,
    pub headers: HashMap<String, String>,
    pub ping_interval: Option<Duration>,
}

/// Events from a WebSocket connection
#[derive(Debug, Clone)]
pub enum WebSocketEvent {
    Connected { protocol: Option<String> },
    Message(WebSocketMessage),
    Error(String),
    Disconnected { code: u16, reason: String },
}

/// Stub for future WebSocket implementation
pub struct WebSocketProtocolHandler {
    // Future: active connections map
}

#[async_trait]
impl ProtocolHandler for WebSocketProtocolHandler {
    fn schemes(&self) -> Vec<ProtocolScheme> {
        vec![ProtocolScheme::new("ws"), ProtocolScheme::new("wss")]
    }

    fn display_name(&self) -> &str {
        "WebSocket"
    }

    async fn execute(
        &self,
        _request: ProtocolRequest,
        _context: &ExecutionContext,
    ) -> Result<ProtocolResponse, ProtocolError> {
        Err(ProtocolError::ProtocolSpecific(
            "WebSocket support not yet implemented".to_string()
        ))
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    async fn cancel(&self, request_id: &str) -> Result<(), ProtocolError> {
        // Future: close WebSocket connection
        Ok(())
    }
}
```

### 6.3 SSE (Server-Sent Events) Handler Interface

```rust
// crates/domain/src/protocol/sse.rs

use super::*;
use async_trait::async_trait;

/// SSE event from server
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// Event type (optional)
    pub event: Option<String>,
    /// Event data
    pub data: String,
    /// Event ID (optional)
    pub id: Option<String>,
    /// Retry interval suggested by server
    pub retry: Option<Duration>,
}

/// SSE connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseState {
    Connecting,
    Open,
    Closed,
}

/// Stub for future SSE implementation
pub struct SseProtocolHandler {
    http_client: reqwest::Client,
}

#[async_trait]
impl ProtocolHandler for SseProtocolHandler {
    fn schemes(&self) -> Vec<ProtocolScheme> {
        // SSE uses HTTP(S) but with special handling
        vec![ProtocolScheme::new("sse"), ProtocolScheme::new("sses")]
    }

    fn display_name(&self) -> &str {
        "Server-Sent Events"
    }

    async fn execute(
        &self,
        _request: ProtocolRequest,
        _context: &ExecutionContext,
    ) -> Result<ProtocolResponse, ProtocolError> {
        Err(ProtocolError::ProtocolSpecific(
            "SSE support not yet implemented".to_string()
        ))
    }

    fn supports_streaming(&self) -> bool {
        true // SSE is streaming by nature
    }
}
```

---

## 7. UI Components

### 7.1 Slint Component Definitions

```slint
// ui/components/plugin_manager.slint

import { Button, ListView, CheckBox, VerticalBox, HorizontalBox } from "std-widgets.slint";

export struct PluginInfo {
    id: string,
    name: string,
    version: string,
    description: string,
    enabled: bool,
    capabilities: string,
}

export component PluginManagerPanel inherits Rectangle {
    in property <[PluginInfo]> plugins;
    in property <bool> loading: false;

    callback enable-plugin(string);
    callback disable-plugin(string);
    callback open-settings(string);
    callback refresh-plugins();

    background: #1e1e1e;

    VerticalBox {
        padding: 16px;
        spacing: 12px;

        // Header
        HorizontalBox {
            alignment: space-between;

            Text {
                text: "Plugins";
                font-size: 18px;
                font-weight: 600;
                color: #ffffff;
            }

            Button {
                text: "Refresh";
                enabled: !loading;
                clicked => { refresh-plugins(); }
            }
        }

        // Plugin list
        ListView {
            for plugin in plugins: PluginListItem {
                plugin-info: plugin;
                toggle-enabled => {
                    if plugin.enabled {
                        disable-plugin(plugin.id);
                    } else {
                        enable-plugin(plugin.id);
                    }
                }
                open-settings => { open-settings(plugin.id); }
            }
        }
    }
}

component PluginListItem inherits Rectangle {
    in property <PluginInfo> plugin-info;

    callback toggle-enabled();
    callback open-settings();

    height: 72px;
    background: #2d2d2d;
    border-radius: 8px;

    HorizontalBox {
        padding: 12px;
        spacing: 12px;

        // Enable checkbox
        CheckBox {
            checked: plugin-info.enabled;
            toggled => { toggle-enabled(); }
        }

        // Plugin info
        VerticalBox {
            spacing: 4px;

            HorizontalBox {
                Text {
                    text: plugin-info.name;
                    font-size: 14px;
                    font-weight: 500;
                    color: #ffffff;
                }
                Text {
                    text: "v" + plugin-info.version;
                    font-size: 12px;
                    color: #888888;
                }
            }

            Text {
                text: plugin-info.description;
                font-size: 12px;
                color: #aaaaaa;
            }

            Text {
                text: plugin-info.capabilities;
                font-size: 11px;
                color: #6b9fff;
            }
        }

        // Settings button
        Button {
            text: "Settings";
            clicked => { open-settings(); }
        }
    }
}
```

```slint
// ui/components/protocol_selector.slint

import { ComboBox, HorizontalBox } from "std-widgets.slint";

export struct ProtocolOption {
    scheme: string,
    display-name: string,
}

export component ProtocolSelector inherits HorizontalBox {
    in property <[ProtocolOption]> protocols;
    in-out property <int> selected-index: 0;

    callback protocol-changed(string);

    spacing: 8px;

    Text {
        text: "Protocol:";
        font-size: 12px;
        color: #888888;
        vertical-alignment: center;
    }

    ComboBox {
        model: protocols.map((p) => p.display-name);
        current-index <=> selected-index;
        selected(value) => {
            protocol-changed(protocols[selected-index].scheme);
        }
    }
}
```

```slint
// ui/components/plugin_settings.slint

import { Button, LineEdit, CheckBox, VerticalBox, HorizontalBox, ScrollView } from "std-widgets.slint";

export struct ConfigFieldDef {
    name: string,
    display-name: string,
    field-type: string, // "string" | "secret" | "integer" | "boolean"
    required: bool,
    description: string,
    current-value: string,
}

export component PluginSettingsPage inherits Rectangle {
    in property <string> plugin-name;
    in property <string> plugin-version;
    in property <[ConfigFieldDef]> config-fields;

    callback save-settings([string]); // values in same order as fields
    callback cancel();

    background: #1e1e1e;

    VerticalBox {
        padding: 24px;
        spacing: 16px;

        // Header
        Text {
            text: plugin-name + " Settings";
            font-size: 20px;
            font-weight: 600;
            color: #ffffff;
        }

        Text {
            text: "Version " + plugin-version;
            font-size: 12px;
            color: #888888;
        }

        // Config fields
        ScrollView {
            VerticalBox {
                spacing: 16px;

                for field in config-fields: ConfigFieldEditor {
                    field-def: field;
                }
            }
        }

        // Actions
        HorizontalBox {
            alignment: end;
            spacing: 8px;

            Button {
                text: "Cancel";
                clicked => { cancel(); }
            }

            Button {
                text: "Save";
                primary: true;
                clicked => {
                    // Collect values and emit
                    save-settings([]);
                }
            }
        }
    }
}

component ConfigFieldEditor inherits VerticalBox {
    in-out property <ConfigFieldDef> field-def;

    spacing: 4px;

    HorizontalBox {
        Text {
            text: field-def.display-name;
            font-size: 13px;
            color: #ffffff;
        }
        if field-def.required: Text {
            text: "*";
            color: #ff6b6b;
        }
    }

    if field-def.field-type == "string" || field-def.field-type == "secret": LineEdit {
        text: field-def.current-value;
        input-type: field-def.field-type == "secret" ? password : text;
    }

    if field-def.description != "": Text {
        text: field-def.description;
        font-size: 11px;
        color: #666666;
    }
}
```

### 7.2 UI State Management

```rust
// crates/ui/src/state/plugin_state.rs

use domain::plugin::{PluginId, PluginCapability};
use infrastructure::plugin::{PluginInfo, PluginState};
use slint::ComponentHandle;
use std::sync::Arc;
use tokio::sync::mpsc;

/// UI state for plugin management
#[derive(Debug, Clone)]
pub struct PluginUiState {
    pub plugins: Vec<PluginInfoUi>,
    pub loading: bool,
    pub selected_plugin: Option<PluginId>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PluginInfoUi {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub capabilities: String,
}

impl From<PluginInfo> for PluginInfoUi {
    fn from(info: PluginInfo) -> Self {
        Self {
            id: info.id.0,
            name: info.name,
            version: info.version,
            description: info.description,
            enabled: info.state == PluginState::Enabled,
            capabilities: info.capabilities
                .iter()
                .map(|c| format!("{:?}", c))
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

/// Commands from UI to plugin system
#[derive(Debug)]
pub enum PluginCommand {
    RefreshList,
    EnablePlugin(PluginId),
    DisablePlugin(PluginId),
    OpenSettings(PluginId),
    SaveSettings(PluginId, serde_json::Value),
}

/// Events from plugin system to UI
#[derive(Debug)]
pub enum PluginEvent {
    ListUpdated(Vec<PluginInfo>),
    PluginEnabled(PluginId),
    PluginDisabled(PluginId),
    Error(String),
}

/// Controller for plugin UI
pub struct PluginUiController {
    command_tx: mpsc::Sender<PluginCommand>,
}

impl PluginUiController {
    pub fn new(command_tx: mpsc::Sender<PluginCommand>) -> Self {
        Self { command_tx }
    }

    pub async fn refresh(&self) {
        let _ = self.command_tx.send(PluginCommand::RefreshList).await;
    }

    pub async fn enable(&self, id: &str) {
        let _ = self.command_tx.send(
            PluginCommand::EnablePlugin(PluginId(id.to_string()))
        ).await;
    }

    pub async fn disable(&self, id: &str) {
        let _ = self.command_tx.send(
            PluginCommand::DisablePlugin(PluginId(id.to_string()))
        ).await;
    }
}
```

---

## 8. Security Considerations

### 8.1 Permission Model

```rust
// crates/domain/src/plugin/security.rs

use super::PluginPermissions;
use std::path::PathBuf;

/// Security policy for plugin execution
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    /// Allow plugins to make network requests
    pub allow_network: bool,
    /// Allowed filesystem read paths (glob patterns)
    pub fs_read_allowlist: Vec<String>,
    /// Allowed filesystem write paths (glob patterns)
    pub fs_write_allowlist: Vec<String>,
    /// Allow access to environment variables
    pub allow_env_vars: bool,
    /// Allow access to stored secrets
    pub allow_secrets: bool,
    /// Maximum execution time per call
    pub max_execution_time: std::time::Duration,
    /// Maximum memory usage
    pub max_memory_bytes: usize,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            allow_network: false,
            fs_read_allowlist: vec![],
            fs_write_allowlist: vec![],
            allow_env_vars: false,
            allow_secrets: false,
            max_execution_time: std::time::Duration::from_secs(30),
            max_memory_bytes: 64 * 1024 * 1024, // 64 MB
        }
    }
}

/// Validates plugin permissions against security policy
pub fn validate_permissions(
    requested: &PluginPermissions,
    policy: &SecurityPolicy,
) -> Result<(), SecurityViolation> {
    // Network access
    if requested.network && !policy.allow_network {
        return Err(SecurityViolation::NetworkAccessDenied);
    }

    // Filesystem read
    for path in &requested.filesystem.read {
        if !is_path_allowed(path, &policy.fs_read_allowlist) {
            return Err(SecurityViolation::FilesystemReadDenied(path.clone()));
        }
    }

    // Filesystem write
    for path in &requested.filesystem.write {
        if !is_path_allowed(path, &policy.fs_write_allowlist) {
            return Err(SecurityViolation::FilesystemWriteDenied(path.clone()));
        }
    }

    // Environment variables
    if requested.environment_variables && !policy.allow_env_vars {
        return Err(SecurityViolation::EnvVarsAccessDenied);
    }

    // Secrets
    if requested.secrets_access && !policy.allow_secrets {
        return Err(SecurityViolation::SecretsAccessDenied);
    }

    Ok(())
}

fn is_path_allowed(requested: &str, allowlist: &[String]) -> bool {
    if allowlist.is_empty() {
        return false;
    }

    for pattern in allowlist {
        if glob_match(pattern, requested) {
            return true;
        }
    }
    false
}

fn glob_match(pattern: &str, path: &str) -> bool {
    // Simplified glob matching - use glob crate in production
    if pattern == "*" {
        return true;
    }
    if pattern.ends_with("/*") {
        let prefix = &pattern[..pattern.len() - 2];
        return path.starts_with(prefix);
    }
    pattern == path
}

#[derive(Debug, thiserror::Error)]
pub enum SecurityViolation {
    #[error("Network access denied")]
    NetworkAccessDenied,
    #[error("Filesystem read denied: {0}")]
    FilesystemReadDenied(String),
    #[error("Filesystem write denied: {0}")]
    FilesystemWriteDenied(String),
    #[error("Environment variables access denied")]
    EnvVarsAccessDenied,
    #[error("Secrets access denied")]
    SecretsAccessDenied,
    #[error("Execution timeout")]
    ExecutionTimeout,
    #[error("Memory limit exceeded")]
    MemoryLimitExceeded,
}
```

### 8.2 Sandboxing Strategy

```rust
// crates/infrastructure/src/plugin/sandbox.rs

use domain::plugin::{PluginManifest, PluginPermissions, PluginError};
use domain::plugin::security::{SecurityPolicy, validate_permissions, SecurityViolation};
use std::sync::Arc;

/// WASM-based sandbox for plugin execution
pub struct WasmSandbox {
    policy: SecurityPolicy,
}

impl WasmSandbox {
    pub fn new(policy: SecurityPolicy) -> Self {
        Self { policy }
    }

    pub fn with_default_policy() -> Self {
        Self::new(SecurityPolicy::default())
    }
}

impl super::PluginSandbox for WasmSandbox {
    fn verify_permissions(&self, permissions: &PluginPermissions) -> Result<(), PluginError> {
        validate_permissions(permissions, &self.policy)
            .map_err(|e| PluginError::PermissionDenied(e.to_string()))
    }

    fn create_runtime(&self, manifest: &PluginManifest) -> Result<Box<dyn super::PluginRuntime>, PluginError> {
        // Future: Create wasmtime runtime with WASI capabilities
        // based on manifest.permissions

        /*
        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::from_file(&engine, &manifest.entry_point)?;

        let mut linker = wasmtime::Linker::new(&engine);

        // Add WASI if filesystem access granted
        if !manifest.permissions.filesystem.read.is_empty() {
            wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;
        }

        // Create store with resource limits
        let mut store = wasmtime::Store::new(&engine, ());
        store.limiter(|_| ResourceLimiter::new(
            self.policy.max_memory_bytes,
            self.policy.max_execution_time,
        ));
        */

        Err(PluginError::LoadFailed(
            "WASM runtime not yet implemented".to_string()
        ))
    }
}

/// Sandboxing considerations for production:
///
/// 1. WASM Isolation:
///    - Each plugin runs in isolated WASM instance
///    - No shared memory between plugins
///    - Cannot access host filesystem without WASI permissions
///    - Network access via explicit host functions only
///
/// 2. Resource Limits:
///    - Memory: Configurable max (default 64MB)
///    - CPU: Fuel-based execution limits in wasmtime
///    - Time: Async timeout wrapping all calls
///
/// 3. Host Functions (imports):
///    - http_request: Only if network permission granted
///    - fs_read: Only allowed paths
///    - fs_write: Only allowed paths
///    - get_variable: Access to request variables
///    - log: Always available, sanitized output
///
/// 4. Future: Signature Verification
///    - Ed25519 signatures on plugin packages
///    - Optional CA for enterprise deployments
///    - Signature check before loading
```

### 8.3 Future Signature Verification

```rust
// crates/infrastructure/src/plugin/signature.rs

/// Plugin signature verification (future feature)
///
/// Package format:
/// plugin.vxpkg (tar.gz containing):
///   - plugin.json (manifest)
///   - plugin.wasm (code)
///   - signature.sig (Ed25519 signature)
///   - other assets...

use ed25519_dalek::{PublicKey, Signature, Verifier};
use sha2::{Sha256, Digest};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SignatureVerifier {
    trusted_keys: Vec<PublicKey>,
}

impl SignatureVerifier {
    pub fn new(trusted_keys: Vec<PublicKey>) -> Self {
        Self { trusted_keys }
    }

    /// Verify plugin package signature
    pub fn verify_package(&self, package_path: &Path) -> Result<(), SignatureError> {
        // 1. Extract manifest and signature
        // 2. Compute hash of all files except signature
        // 3. Verify signature against hash with any trusted key

        Err(SignatureError::NotImplemented)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("Signature verification not implemented")]
    NotImplemented,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("No trusted key matched")]
    UntrustedKey,
    #[error("Package corrupted: {0}")]
    CorruptedPackage(String),
}
```

---

## 9. Integration: Registering Plugins with Protocol Registry

```rust
// crates/application/src/plugin/integration.rs

use crate::protocol::registry::ProtocolRegistry;
use domain::plugin::{PluginCapability, PluginId};
use infrastructure::plugin::{PluginManager, LoadedPlugin, PluginState};
use std::sync::Arc;

/// Integrates plugin system with protocol registry
pub struct PluginProtocolIntegration {
    plugin_manager: Arc<PluginManager>,
    protocol_registry: Arc<ProtocolRegistry>,
}

impl PluginProtocolIntegration {
    pub fn new(
        plugin_manager: Arc<PluginManager>,
        protocol_registry: Arc<ProtocolRegistry>,
    ) -> Self {
        Self {
            plugin_manager,
            protocol_registry,
        }
    }

    /// Register all enabled protocol plugins with the registry
    pub async fn register_protocol_plugins(&self) -> Result<(), IntegrationError> {
        let protocol_plugins = self.plugin_manager
            .get_by_capability(PluginCapability::Protocol)
            .await;

        for plugin_id in protocol_plugins {
            if let Err(e) = self.register_plugin_protocol(&plugin_id).await {
                tracing::warn!(
                    "Failed to register protocol from plugin {:?}: {}",
                    plugin_id, e
                );
            }
        }

        Ok(())
    }

    /// Register a single plugin's protocol handler
    async fn register_plugin_protocol(&self, plugin_id: &PluginId) -> Result<(), IntegrationError> {
        // Future: Create bridge from WASM plugin to ProtocolHandler trait
        // This would involve:
        // 1. Loading the WASM module
        // 2. Creating a WasmProtocolAdapter that implements ProtocolHandler
        // 3. Registering the adapter with the protocol registry

        tracing::info!("Registered protocol handler from plugin: {:?}", plugin_id);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IntegrationError {
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),
    #[error("Plugin does not provide protocol capability")]
    NotProtocolPlugin,
    #[error("Failed to create protocol handler: {0}")]
    HandlerCreationFailed(String),
}
```

---

## 10. Implementation Order

### Phase 1: Core Protocol Abstraction (Days 1-3)

1. **Create domain types** (Day 1)
   - [ ] Define `ProtocolRequest`, `ProtocolResponse` in `crates/domain/src/protocol/mod.rs`
   - [ ] Define `ProtocolHandler` trait
   - [ ] Define `ProtocolScheme` and associated types
   - [ ] Define `ProtocolError` enum

2. **Implement Protocol Registry** (Day 2)
   - [ ] Create `ProtocolRegistry` in `crates/application/src/protocol/registry.rs`
   - [ ] Implement register/unregister/get methods
   - [ ] Add thread-safe concurrent access with RwLock
   - [ ] Write unit tests for registry

3. **HTTP Default Implementation** (Day 3)
   - [ ] Implement `HttpProtocolHandler` in `crates/infrastructure/src/protocol/http.rs`
   - [ ] Integrate with existing reqwest client code
   - [ ] Register HTTP as default handler
   - [ ] Integration tests with real HTTP endpoints

### Phase 2: Plugin System Foundation (Days 4-6)

4. **Plugin Manifest** (Day 4)
   - [ ] Define `PluginManifest` struct in `crates/domain/src/plugin/manifest.rs`
   - [ ] Define `PluginCapability` enum
   - [ ] Define `PluginPermissions` struct
   - [ ] Implement JSON deserialization with validation

5. **Plugin Loader** (Day 5)
   - [ ] Implement `PluginLoader` in `crates/infrastructure/src/plugin/loader.rs`
   - [ ] Discover plugins from configured directories
   - [ ] Parse and validate manifests
   - [ ] Handle filesystem errors gracefully

6. **Plugin Manager** (Day 6)
   - [ ] Implement `PluginManager` for lifecycle management
   - [ ] Add load/enable/disable/unload operations
   - [ ] Track plugin states
   - [ ] Emit events for UI updates

### Phase 3: Security Layer (Days 7-8)

7. **Permission System** (Day 7)
   - [ ] Implement `SecurityPolicy` in `crates/domain/src/plugin/security.rs`
   - [ ] Create permission validation logic
   - [ ] Define security violation errors
   - [ ] Unit tests for permission checks

8. **Sandbox Foundation** (Day 8)
   - [ ] Define `PluginSandbox` trait
   - [ ] Implement `WasmSandbox` stub
   - [ ] Document sandbox architecture
   - [ ] Create placeholder for wasmtime integration

### Phase 4: Mock Plugin Example (Days 9-10)

9. **Mock Protocol Implementation** (Day 9)
   - [ ] Implement `MockProtocolHandler` in `crates/infrastructure/src/protocol/mock.rs`
   - [ ] Create configurable routes
   - [ ] Add delay simulation
   - [ ] Comprehensive tests

10. **Plugin Package** (Day 10)
    - [ ] Create `plugins/mock-protocol/` directory structure
    - [ ] Write `plugin.json` manifest
    - [ ] Create README documentation
    - [ ] End-to-end test: load plugin and execute mock request

### Phase 5: UI Components (Days 11-12)

11. **Plugin Manager Panel** (Day 11)
    - [ ] Create `plugin_manager.slint` component
    - [ ] Implement plugin list view
    - [ ] Add enable/disable toggles
    - [ ] Wire up refresh functionality

12. **Protocol Selector & Settings** (Day 12)
    - [ ] Create `protocol_selector.slint` component
    - [ ] Add protocol dropdown to request editor
    - [ ] Create `plugin_settings.slint` page
    - [ ] Implement settings persistence

### Phase 6: Future Protocol Stubs (Days 13-14)

13. **gRPC Stub** (Day 13)
    - [ ] Define gRPC-specific types in `crates/domain/src/protocol/grpc.rs`
    - [ ] Create `GrpcProtocolHandler` stub
    - [ ] Document protobuf integration plan
    - [ ] Add UI placeholder for proto file upload

14. **WebSocket & SSE Stubs** (Day 14)
    - [ ] Define WebSocket types in `crates/domain/src/protocol/websocket.rs`
    - [ ] Define SSE types in `crates/domain/src/protocol/sse.rs`
    - [ ] Create stub handlers
    - [ ] Document connection lifecycle management

---

## Checklist (Summary)

- [ ] ProtocolRequest/ProtocolResponse types defined
- [ ] ProtocolHandler trait with async execute
- [ ] ProtocolRegistry with register/get methods
- [ ] HTTP handler as default implementation
- [ ] Plugin manifest schema (plugin.json)
- [ ] PluginCapability enum (Protocol, Auth, Importer, Exporter)
- [ ] PluginLoader for discovery
- [ ] PluginManager for lifecycle
- [ ] PluginPermissions and SecurityPolicy
- [ ] WasmSandbox placeholder
- [ ] MockProtocolHandler example
- [ ] Mock plugin package structure
- [ ] Plugin manager UI panel
- [ ] Protocol selector in request editor
- [ ] Plugin settings page
- [ ] gRPC handler stub
- [ ] WebSocket handler stub
- [ ] SSE handler stub
- [ ] Integration tests passing
- [ ] Documentation complete

---

## Acceptance Criteria

1. **Protocol Abstraction**: New protocols can be added by implementing the `ProtocolHandler` trait
2. **Plugin Loading**: Plugins are discovered from filesystem and manifests validated
3. **Security**: Plugin permissions are validated against security policy before loading
4. **Mock Plugin**: Mock protocol returns configurable responses for testing
5. **UI Integration**: Users can view, enable, and disable plugins from the UI
6. **Future Ready**: gRPC/WebSocket/SSE stubs provide clear extension points

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| WASM runtime complexity | High | Start with native plugins, add WASM later |
| Plugin API instability | Medium | Version manifest format, maintain compatibility |
| Security vulnerabilities | High | Strict permission model, code review, fuzzing |
| Performance overhead | Medium | Profile plugin calls, optimize hot paths |
| Platform differences | Medium | Use cross-platform WASI APIs |

---

## Dependencies

- `async-trait` - Async trait methods
- `semver` - Version parsing
- `thiserror` - Error derive macros
- `serde` / `serde_json` - Manifest parsing
- `tokio` - Async runtime
- `tracing` - Logging
- `reqwest` - HTTP client (existing)
- `wasmtime` (future) - WASM runtime
- `ed25519-dalek` (future) - Signature verification

---

## Related Milestones

- **M7**: Extensibility and Plugins (this sprint)
- **M8** (future): gRPC Support
- **M9** (future): WebSocket Support

---

## References

- [Slint Documentation](https://slint.dev/docs)
- [wasmtime WASI](https://docs.wasmtime.dev/)
- [Plugin Architecture Patterns](https://www.thoughtworks.com/insights/blog/architecture/plugin-architecture)
