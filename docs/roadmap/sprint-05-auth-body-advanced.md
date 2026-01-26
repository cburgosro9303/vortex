# Sprint 05 — Advanced Authentication and Body Types

**Objective:** Implement complete OAuth2 flows, all body types, and TLS configuration for production-ready API requests.

**Duration:** 2 weeks

**Prerequisites:** Sprint 04 completed (Postman import working)

---

## Scope

### In Scope
- OAuth2 Client Credentials flow with automatic token refresh
- OAuth2 Authorization Code flow with local callback server
- In-memory token storage with expiry tracking
- Form URL-encoded body serialization
- Multipart form-data with file uploads
- Binary body (raw file content)
- GraphQL body (query + variables)
- Custom CA certificates
- Client certificates (mTLS)
- Insecure mode with explicit warnings
- Certificate validation error handling

### Out of Scope
- OAuth2 PKCE extension (Sprint 06)
- OAuth2 Device Code flow
- Persistent token storage (keychain integration)
- WebSocket connections
- gRPC protocol support

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              UI Layer                                    │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │
│  │ AuthTypeSelector│  │ BodyTypeEditor  │  │ TlsSettingsPanel       │  │
│  │ OAuth2FlowPanel │  │ FormDataEditor  │  │ CertificateManager     │  │
│  │ TokenStatusBar  │  │ GraphQLEditor   │  │                        │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────┘  │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │ Commands / Events
┌──────────────────────────────┴──────────────────────────────────────────┐
│                         Application Layer                                │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │
│  │ AuthUseCase     │  │ BodyBuilder     │  │ TlsConfigUseCase       │  │
│  │ TokenManager    │  │ UseCase         │  │                        │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────┘  │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────────────────────┐
│                          Domain Layer                                    │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │
│  │ AuthSpec        │  │ RequestBody     │  │ TlsConfig              │  │
│  │ OAuth2Token     │  │ FormField       │  │ CertificateInfo        │  │
│  │ TokenStore      │  │ GraphQLQuery    │  │                        │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────┘  │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────────────────────┐
│                       Infrastructure Layer                               │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │
│  │ OAuth2Provider  │  │ ReqwestBody     │  │ TlsConfigBuilder       │  │
│  │ (oauth2 crate)  │  │ Builder         │  │ (rustls/native-tls)    │  │
│  │ CallbackServer  │  │                 │  │                        │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Dependencies (Cargo.toml)

```toml
[workspace.dependencies]
# OAuth2
oauth2 = "4.4"

# HTTP client with multipart support
reqwest = { version = "0.12", features = ["json", "multipart", "stream"] }

# TLS
rustls = "0.23"
rustls-pemfile = "2.1"
webpki-roots = "0.26"
native-tls = "0.2"  # Fallback for system certificates

# Async runtime
tokio = { version = "1.37", features = ["full", "net"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_urlencoded = "0.7"

# Utilities
url = "2.5"
base64 = "0.22"
uuid = { version = "1.8", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"
mime = "0.3"
mime_guess = "2.0"
```

---

## Part 1: Domain Layer Types

### File: `crates/domain/src/auth.rs`

```rust
//! Authentication domain types for Vortex API Client.
//!
//! This module defines all authentication-related types that are protocol-agnostic
//! and can be serialized to/from the Vortex file format.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Authentication specification for a request.
/// Matches the file format spec from `02-file-format-spec.md`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthSpec {
    /// No authentication
    None,

    /// Bearer token authentication
    Bearer {
        /// The token value (may contain variables like `{{access_token}}`)
        token: String,
        /// Optional prefix, defaults to "Bearer"
        #[serde(default = "default_bearer_prefix")]
        prefix: String,
    },

    /// HTTP Basic authentication
    Basic {
        /// Username (may contain variables)
        username: String,
        /// Password (may contain variables)
        password: String,
    },

    /// API Key authentication
    ApiKey {
        /// Header or query parameter name
        key: String,
        /// The API key value (may contain variables)
        value: String,
        /// Where to send the key
        location: ApiKeyLocation,
    },

    /// OAuth2 Client Credentials flow
    #[serde(rename = "oauth2_client_credentials")]
    OAuth2ClientCredentials {
        /// Token endpoint URL
        token_url: String,
        /// Client ID
        client_id: String,
        /// Client secret
        client_secret: String,
        /// Space-separated scopes
        #[serde(default)]
        scope: Option<String>,
        /// Additional parameters to send with token request
        #[serde(default)]
        extra_params: HashMap<String, String>,
    },

    /// OAuth2 Authorization Code flow
    #[serde(rename = "oauth2_auth_code")]
    OAuth2AuthorizationCode {
        /// Authorization endpoint URL
        auth_url: String,
        /// Token endpoint URL
        token_url: String,
        /// Client ID
        client_id: String,
        /// Client secret
        client_secret: String,
        /// Redirect URI for the callback
        redirect_uri: String,
        /// Space-separated scopes
        #[serde(default)]
        scope: Option<String>,
        /// Additional parameters for authorization request
        #[serde(default)]
        extra_params: HashMap<String, String>,
    },
}

fn default_bearer_prefix() -> String {
    "Bearer".to_string()
}

/// Location for API key placement
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyLocation {
    #[default]
    Header,
    Query,
}

/// OAuth2 token with metadata for expiry tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Token {
    /// Unique identifier for this token
    pub id: Uuid,
    /// The access token string
    pub access_token: String,
    /// Token type (usually "Bearer")
    pub token_type: String,
    /// When the token expires (if known)
    pub expires_at: Option<DateTime<Utc>>,
    /// Refresh token for obtaining new access tokens
    pub refresh_token: Option<String>,
    /// Scopes granted by this token
    pub scopes: Vec<String>,
    /// When this token was obtained
    pub obtained_at: DateTime<Utc>,
    /// Key identifying which auth config this token belongs to
    pub auth_config_key: String,
}

impl OAuth2Token {
    /// Check if the token is expired or will expire within the given buffer
    pub fn is_expired_or_expiring(&self, buffer_seconds: i64) -> bool {
        match self.expires_at {
            Some(expires_at) => {
                let buffer = chrono::Duration::seconds(buffer_seconds);
                Utc::now() + buffer >= expires_at
            }
            None => false, // No expiry known, assume valid
        }
    }

    /// Check if the token can be refreshed
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }

    /// Time until expiry in seconds, or None if no expiry
    pub fn seconds_until_expiry(&self) -> Option<i64> {
        self.expires_at.map(|exp| (exp - Utc::now()).num_seconds())
    }
}

/// Result of an authentication resolution
#[derive(Debug, Clone)]
pub enum AuthResolution {
    /// No authentication needed
    None,
    /// Add this header to the request
    Header { name: String, value: String },
    /// Add this query parameter
    QueryParam { name: String, value: String },
    /// Authentication is pending (e.g., waiting for OAuth callback)
    Pending { message: String },
    /// Authentication failed
    Failed { error: AuthError },
}

/// Authentication errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum AuthError {
    #[error("Token expired and no refresh token available")]
    TokenExpiredNoRefresh,

    #[error("Failed to refresh token: {message}")]
    RefreshFailed { message: String },

    #[error("OAuth2 authorization failed: {message}")]
    OAuth2AuthorizationFailed { message: String },

    #[error("Invalid OAuth2 configuration: {message}")]
    InvalidConfiguration { message: String },

    #[error("User cancelled authentication")]
    UserCancelled,

    #[error("Callback server error: {message}")]
    CallbackServerError { message: String },

    #[error("Network error: {message}")]
    NetworkError { message: String },
}
```

### File: `crates/domain/src/body.rs`

```rust
//! Request body domain types for Vortex API Client.
//!
//! Supports all body types from the file format spec.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Request body specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequestBody {
    /// No body
    None,

    /// JSON body
    Json {
        /// JSON content (can be any valid JSON value)
        content: serde_json::Value,
    },

    /// Plain text body
    Text {
        /// Text content (may contain variables)
        content: String,
    },

    /// URL-encoded form body (application/x-www-form-urlencoded)
    FormUrlencoded {
        /// Key-value pairs to encode
        fields: HashMap<String, String>,
    },

    /// Multipart form data (multipart/form-data)
    FormData {
        /// Form fields (text or file)
        fields: Vec<FormDataField>,
    },

    /// Raw binary content from a file
    Binary {
        /// Path to the file (relative to workspace or absolute)
        path: PathBuf,
        /// Optional content type override
        #[serde(default)]
        content_type: Option<String>,
    },

    /// GraphQL query
    GraphQL {
        /// The GraphQL query string
        query: String,
        /// Optional variables for the query
        #[serde(default)]
        variables: Option<serde_json::Value>,
        /// Optional operation name
        #[serde(default)]
        operation_name: Option<String>,
    },
}

impl Default for RequestBody {
    fn default() -> Self {
        Self::None
    }
}

/// A field in a multipart form
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FormDataField {
    /// Text field
    Text {
        /// Field name
        name: String,
        /// Field value (may contain variables)
        value: String,
    },
    /// File field
    File {
        /// Field name
        name: String,
        /// Path to the file
        path: PathBuf,
        /// Optional filename override
        #[serde(default)]
        filename: Option<String>,
        /// Optional content type override
        #[serde(default)]
        content_type: Option<String>,
    },
}

impl FormDataField {
    /// Get the field name
    pub fn name(&self) -> &str {
        match self {
            FormDataField::Text { name, .. } => name,
            FormDataField::File { name, .. } => name,
        }
    }
}

/// Content type for request bodies
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyContentType {
    Json,
    Text,
    FormUrlencoded,
    FormData,
    Binary(String), // Custom MIME type
}

impl BodyContentType {
    /// Get the MIME type string
    pub fn as_mime(&self) -> &str {
        match self {
            Self::Json => "application/json",
            Self::Text => "text/plain",
            Self::FormUrlencoded => "application/x-www-form-urlencoded",
            Self::FormData => "multipart/form-data",
            Self::Binary(mime) => mime,
        }
    }
}

impl RequestBody {
    /// Get the content type for this body
    pub fn content_type(&self) -> Option<BodyContentType> {
        match self {
            Self::None => None,
            Self::Json { .. } => Some(BodyContentType::Json),
            Self::Text { .. } => Some(BodyContentType::Text),
            Self::FormUrlencoded { .. } => Some(BodyContentType::FormUrlencoded),
            Self::FormData { .. } => Some(BodyContentType::FormData),
            Self::Binary { content_type, path } => {
                let mime = content_type
                    .clone()
                    .unwrap_or_else(|| guess_mime_type(path));
                Some(BodyContentType::Binary(mime))
            }
            Self::GraphQL { .. } => Some(BodyContentType::Json),
        }
    }

    /// Check if this body type requires a file to exist
    pub fn required_files(&self) -> Vec<&PathBuf> {
        match self {
            Self::Binary { path, .. } => vec![path],
            Self::FormData { fields } => fields
                .iter()
                .filter_map(|f| match f {
                    FormDataField::File { path, .. } => Some(path),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        }
    }
}

/// Guess MIME type from file extension
fn guess_mime_type(path: &PathBuf) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string()
}

/// GraphQL request structure for JSON serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLRequest {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "operationName")]
    pub operation_name: Option<String>,
}

impl From<&RequestBody> for Option<GraphQLRequest> {
    fn from(body: &RequestBody) -> Self {
        match body {
            RequestBody::GraphQL {
                query,
                variables,
                operation_name,
            } => Some(GraphQLRequest {
                query: query.clone(),
                variables: variables.clone(),
                operation_name: operation_name.clone(),
            }),
            _ => None,
        }
    }
}
```

### File: `crates/domain/src/tls.rs`

```rust
//! TLS configuration domain types for Vortex API Client.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// TLS configuration for HTTP requests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TlsConfig {
    /// Whether to verify server certificates
    #[serde(default = "default_true")]
    pub verify_certificates: bool,

    /// Custom CA certificates to trust
    #[serde(default)]
    pub ca_certificates: Vec<CertificateSource>,

    /// Client certificate for mTLS
    #[serde(default)]
    pub client_certificate: Option<ClientCertificate>,

    /// Minimum TLS version (default: TLS 1.2)
    #[serde(default)]
    pub min_tls_version: Option<TlsVersion>,

    /// Accept invalid/self-signed certificates (dangerous!)
    #[serde(default)]
    pub danger_accept_invalid_certs: bool,

    /// Accept invalid hostnames (dangerous!)
    #[serde(default)]
    pub danger_accept_invalid_hostnames: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            verify_certificates: true,
            ca_certificates: vec![],
            client_certificate: None,
            min_tls_version: None,
            danger_accept_invalid_certs: false,
            danger_accept_invalid_hostnames: false,
        }
    }
}

impl TlsConfig {
    /// Check if this config uses any dangerous/insecure options
    pub fn has_security_warnings(&self) -> Vec<TlsSecurityWarning> {
        let mut warnings = vec![];

        if !self.verify_certificates {
            warnings.push(TlsSecurityWarning::CertificateVerificationDisabled);
        }

        if self.danger_accept_invalid_certs {
            warnings.push(TlsSecurityWarning::AcceptingInvalidCertificates);
        }

        if self.danger_accept_invalid_hostnames {
            warnings.push(TlsSecurityWarning::AcceptingInvalidHostnames);
        }

        warnings
    }

    /// Check if this is a secure configuration
    pub fn is_secure(&self) -> bool {
        self.has_security_warnings().is_empty()
    }
}

/// Source for a certificate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CertificateSource {
    /// Load from a PEM file
    PemFile { path: PathBuf },
    /// Load from a DER file
    DerFile { path: PathBuf },
    /// Inline PEM content
    PemContent { content: String },
    /// Use system certificate store
    System,
}

/// Client certificate for mTLS authentication
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientCertificate {
    /// Certificate source
    pub certificate: CertificateSource,
    /// Private key source
    pub private_key: PrivateKeySource,
    /// Password for encrypted keys (if applicable)
    #[serde(default)]
    pub password: Option<String>,
}

/// Source for a private key
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PrivateKeySource {
    /// Load from a PEM file
    PemFile { path: PathBuf },
    /// Load from a DER file
    DerFile { path: PathBuf },
    /// Load from a PKCS#12 file (includes cert)
    Pkcs12File { path: PathBuf },
    /// Inline PEM content
    PemContent { content: String },
}

/// TLS protocol version
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TlsVersion {
    #[serde(rename = "1.0")]
    Tls10,
    #[serde(rename = "1.1")]
    Tls11,
    #[serde(rename = "1.2")]
    Tls12,
    #[serde(rename = "1.3")]
    Tls13,
}

/// TLS security warnings
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsSecurityWarning {
    CertificateVerificationDisabled,
    AcceptingInvalidCertificates,
    AcceptingInvalidHostnames,
}

impl TlsSecurityWarning {
    /// Get a user-friendly message for this warning
    pub fn message(&self) -> &'static str {
        match self {
            Self::CertificateVerificationDisabled => {
                "Certificate verification is disabled. This makes connections vulnerable to \
                 man-in-the-middle attacks."
            }
            Self::AcceptingInvalidCertificates => {
                "Accepting invalid certificates. Connections may be intercepted by attackers."
            }
            Self::AcceptingInvalidHostnames => {
                "Accepting invalid hostnames. The server identity is not being verified."
            }
        }
    }

    /// Get the severity level
    pub fn severity(&self) -> WarningSeverity {
        WarningSeverity::High
    }
}

/// Severity level for warnings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningSeverity {
    Low,
    Medium,
    High,
}

/// Information about a loaded certificate
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    /// Subject common name
    pub subject_cn: Option<String>,
    /// Issuer common name
    pub issuer_cn: Option<String>,
    /// Serial number (hex)
    pub serial_number: String,
    /// Not valid before
    pub not_before: chrono::DateTime<chrono::Utc>,
    /// Not valid after
    pub not_after: chrono::DateTime<chrono::Utc>,
    /// Whether this is a CA certificate
    pub is_ca: bool,
    /// Fingerprint (SHA-256)
    pub fingerprint_sha256: String,
}

impl CertificateInfo {
    /// Check if the certificate is currently valid
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now();
        now >= self.not_before && now <= self.not_after
    }

    /// Days until expiry (negative if expired)
    pub fn days_until_expiry(&self) -> i64 {
        (self.not_after - chrono::Utc::now()).num_days()
    }
}
```

---

## Part 2: Application Layer

### File: `crates/application/src/auth/token_store.rs`

```rust
//! In-memory token storage with automatic refresh scheduling.

use crate::domain::auth::{AuthError, OAuth2Token};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Thread-safe in-memory token store
#[derive(Debug, Clone)]
pub struct TokenStore {
    tokens: Arc<RwLock<HashMap<String, OAuth2Token>>>,
    /// Seconds before expiry to trigger refresh
    refresh_buffer_seconds: i64,
}

impl TokenStore {
    /// Create a new token store with default settings
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            refresh_buffer_seconds: 60, // Refresh 60 seconds before expiry
        }
    }

    /// Create with custom refresh buffer
    pub fn with_refresh_buffer(refresh_buffer_seconds: i64) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            refresh_buffer_seconds,
        }
    }

    /// Store a token with the given key
    pub async fn store(&self, key: String, token: OAuth2Token) {
        let mut tokens = self.tokens.write().await;
        tokens.insert(key, token);
    }

    /// Get a token by key, returns None if not found or expired
    pub async fn get(&self, key: &str) -> Option<OAuth2Token> {
        let tokens = self.tokens.read().await;
        tokens.get(key).cloned()
    }

    /// Get a valid (non-expired) token, or None if expired/missing
    pub async fn get_valid(&self, key: &str) -> Option<OAuth2Token> {
        let tokens = self.tokens.read().await;
        tokens.get(key).and_then(|t| {
            if t.is_expired_or_expiring(0) {
                None
            } else {
                Some(t.clone())
            }
        })
    }

    /// Check if a token needs refresh (exists but expiring soon)
    pub async fn needs_refresh(&self, key: &str) -> bool {
        let tokens = self.tokens.read().await;
        tokens
            .get(key)
            .map(|t| t.is_expired_or_expiring(self.refresh_buffer_seconds) && t.can_refresh())
            .unwrap_or(false)
    }

    /// Remove a token
    pub async fn remove(&self, key: &str) -> Option<OAuth2Token> {
        let mut tokens = self.tokens.write().await;
        tokens.remove(key)
    }

    /// Clear all tokens
    pub async fn clear(&self) {
        let mut tokens = self.tokens.write().await;
        tokens.clear();
    }

    /// Get all token keys
    pub async fn keys(&self) -> Vec<String> {
        let tokens = self.tokens.read().await;
        tokens.keys().cloned().collect()
    }

    /// Get token status for UI display
    pub async fn get_status(&self, key: &str) -> TokenStatus {
        let tokens = self.tokens.read().await;
        match tokens.get(key) {
            None => TokenStatus::NotAuthenticated,
            Some(token) => {
                if token.is_expired_or_expiring(0) {
                    TokenStatus::Expired {
                        can_refresh: token.can_refresh(),
                    }
                } else if token.is_expired_or_expiring(self.refresh_buffer_seconds) {
                    TokenStatus::ExpiringSoon {
                        seconds_remaining: token.seconds_until_expiry().unwrap_or(0),
                    }
                } else {
                    TokenStatus::Valid {
                        seconds_remaining: token.seconds_until_expiry(),
                    }
                }
            }
        }
    }
}

impl Default for TokenStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Token status for UI display
#[derive(Debug, Clone, PartialEq)]
pub enum TokenStatus {
    NotAuthenticated,
    Valid { seconds_remaining: Option<i64> },
    ExpiringSoon { seconds_remaining: i64 },
    Expired { can_refresh: bool },
    Refreshing,
    Error { message: String },
}

impl TokenStatus {
    /// Get a display string for the status
    pub fn display(&self) -> String {
        match self {
            Self::NotAuthenticated => "Not authenticated".to_string(),
            Self::Valid {
                seconds_remaining: Some(secs),
            } => {
                let mins = secs / 60;
                if mins > 60 {
                    format!("Valid ({:.1}h remaining)", mins as f64 / 60.0)
                } else {
                    format!("Valid ({} min remaining)", mins)
                }
            }
            Self::Valid {
                seconds_remaining: None,
            } => "Valid (no expiry)".to_string(),
            Self::ExpiringSoon { seconds_remaining } => {
                format!("Expiring soon ({} sec)", seconds_remaining)
            }
            Self::Expired { can_refresh: true } => "Expired (refresh available)".to_string(),
            Self::Expired { can_refresh: false } => "Expired (re-auth required)".to_string(),
            Self::Refreshing => "Refreshing...".to_string(),
            Self::Error { message } => format!("Error: {}", message),
        }
    }
}
```

### File: `crates/application/src/auth/provider.rs`

```rust
//! Authentication provider trait and implementations.

use crate::domain::auth::{AuthError, AuthResolution, AuthSpec, OAuth2Token};
use async_trait::async_trait;
use std::sync::Arc;

/// Trait for authentication providers
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Resolve authentication to headers/params for a request
    async fn resolve(&self, spec: &AuthSpec) -> Result<AuthResolution, AuthError>;

    /// Start an OAuth2 authorization flow (for auth code flow)
    async fn start_authorization(&self, spec: &AuthSpec) -> Result<AuthorizationState, AuthError>;

    /// Handle OAuth2 callback
    async fn handle_callback(
        &self,
        state: &AuthorizationState,
        code: &str,
    ) -> Result<OAuth2Token, AuthError>;

    /// Refresh an existing token
    async fn refresh_token(&self, token: &OAuth2Token) -> Result<OAuth2Token, AuthError>;
}

/// State for an ongoing authorization flow
#[derive(Debug, Clone)]
pub struct AuthorizationState {
    /// The authorization URL to open in browser
    pub auth_url: String,
    /// CSRF state parameter
    pub state: String,
    /// PKCE verifier (if using PKCE)
    pub pkce_verifier: Option<String>,
    /// Expected redirect URI
    pub redirect_uri: String,
    /// The original auth spec
    pub auth_spec_key: String,
}

/// Events emitted during authentication
#[derive(Debug, Clone)]
pub enum AuthEvent {
    /// Authorization URL ready - open in browser
    AuthorizationUrlReady { url: String },
    /// Waiting for callback
    WaitingForCallback,
    /// Callback received, exchanging code
    ExchangingCode,
    /// Token obtained successfully
    TokenObtained { expires_in: Option<i64> },
    /// Token refreshed
    TokenRefreshed { expires_in: Option<i64> },
    /// Error occurred
    Error { message: String },
}

/// Listener for auth events (for UI updates)
pub type AuthEventListener = Arc<dyn Fn(AuthEvent) + Send + Sync>;
```

---

## Part 3: Infrastructure Layer

### File: `crates/infrastructure/src/auth/oauth2_provider.rs`

```rust
//! OAuth2 provider implementation using the oauth2 crate.

use crate::application::auth::{AuthEventListener, AuthorizationState, AuthProvider};
use crate::application::auth::token_store::TokenStore;
use crate::domain::auth::{AuthError, AuthResolution, AuthSpec, OAuth2Token};
use async_trait::async_trait;
use chrono::Utc;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// OAuth2 authentication provider
pub struct OAuth2Provider {
    token_store: TokenStore,
    event_listener: Option<AuthEventListener>,
    pending_authorizations: Arc<RwLock<std::collections::HashMap<String, AuthorizationState>>>,
}

impl OAuth2Provider {
    pub fn new(token_store: TokenStore) -> Self {
        Self {
            token_store,
            event_listener: None,
            pending_authorizations: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub fn with_event_listener(mut self, listener: AuthEventListener) -> Self {
        self.event_listener = Some(listener);
        self
    }

    fn emit_event(&self, event: crate::application::auth::AuthEvent) {
        if let Some(ref listener) = self.event_listener {
            listener(event);
        }
    }

    /// Generate a unique key for an auth spec
    fn auth_spec_key(spec: &AuthSpec) -> String {
        match spec {
            AuthSpec::OAuth2ClientCredentials {
                token_url,
                client_id,
                ..
            } => format!("cc:{}:{}", token_url, client_id),
            AuthSpec::OAuth2AuthorizationCode {
                auth_url,
                client_id,
                ..
            } => format!("ac:{}:{}", auth_url, client_id),
            _ => String::new(),
        }
    }

    /// Fetch a new token using client credentials flow
    async fn fetch_client_credentials_token(
        &self,
        token_url: &str,
        client_id: &str,
        client_secret: &str,
        scope: Option<&str>,
        extra_params: &std::collections::HashMap<String, String>,
    ) -> Result<OAuth2Token, AuthError> {
        let client = BasicClient::new(
            ClientId::new(client_id.to_string()),
            Some(ClientSecret::new(client_secret.to_string())),
            // Auth URL not needed for client credentials, but required by the API
            AuthUrl::new("https://unused.example.com/auth".to_string())
                .map_err(|e| AuthError::InvalidConfiguration {
                    message: e.to_string(),
                })?,
            Some(
                TokenUrl::new(token_url.to_string()).map_err(|e| {
                    AuthError::InvalidConfiguration {
                        message: e.to_string(),
                    }
                })?,
            ),
        );

        let mut request = client.exchange_client_credentials();

        // Add scopes
        if let Some(scope_str) = scope {
            for s in scope_str.split_whitespace() {
                request = request.add_scope(Scope::new(s.to_string()));
            }
        }

        // Add extra parameters
        for (key, value) in extra_params {
            request = request.add_extra_param(key, value);
        }

        let token_result = request
            .request_async(async_http_client)
            .await
            .map_err(|e| AuthError::NetworkError {
                message: e.to_string(),
            })?;

        let expires_at = token_result.expires_in().map(|d| {
            Utc::now() + chrono::Duration::seconds(d.as_secs() as i64)
        });

        let scopes: Vec<String> = token_result
            .scopes()
            .map(|s| s.iter().map(|sc| sc.to_string()).collect())
            .unwrap_or_default();

        Ok(OAuth2Token {
            id: Uuid::new_v4(),
            access_token: token_result.access_token().secret().clone(),
            token_type: "Bearer".to_string(),
            expires_at,
            refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
            scopes,
            obtained_at: Utc::now(),
            auth_config_key: format!("cc:{}:{}", token_url, client_id),
        })
    }
}

#[async_trait]
impl AuthProvider for OAuth2Provider {
    async fn resolve(&self, spec: &AuthSpec) -> Result<AuthResolution, AuthError> {
        match spec {
            AuthSpec::None => Ok(AuthResolution::None),

            AuthSpec::Bearer { token, prefix } => Ok(AuthResolution::Header {
                name: "Authorization".to_string(),
                value: format!("{} {}", prefix, token),
            }),

            AuthSpec::Basic { username, password } => {
                let credentials = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    format!("{}:{}", username, password),
                );
                Ok(AuthResolution::Header {
                    name: "Authorization".to_string(),
                    value: format!("Basic {}", credentials),
                })
            }

            AuthSpec::ApiKey {
                key,
                value,
                location,
            } => match location {
                crate::domain::auth::ApiKeyLocation::Header => Ok(AuthResolution::Header {
                    name: key.clone(),
                    value: value.clone(),
                }),
                crate::domain::auth::ApiKeyLocation::Query => Ok(AuthResolution::QueryParam {
                    name: key.clone(),
                    value: value.clone(),
                }),
            },

            AuthSpec::OAuth2ClientCredentials {
                token_url,
                client_id,
                client_secret,
                scope,
                extra_params,
            } => {
                let key = Self::auth_spec_key(spec);

                // Check if we have a valid token
                if let Some(token) = self.token_store.get_valid(&key).await {
                    return Ok(AuthResolution::Header {
                        name: "Authorization".to_string(),
                        value: format!("Bearer {}", token.access_token),
                    });
                }

                // Check if we need to refresh
                if self.token_store.needs_refresh(&key).await {
                    if let Some(token) = self.token_store.get(&key).await {
                        match self.refresh_token(&token).await {
                            Ok(new_token) => {
                                let access_token = new_token.access_token.clone();
                                self.token_store.store(key, new_token).await;
                                return Ok(AuthResolution::Header {
                                    name: "Authorization".to_string(),
                                    value: format!("Bearer {}", access_token),
                                });
                            }
                            Err(_) => {
                                // Refresh failed, try to get a new token
                            }
                        }
                    }
                }

                // Fetch a new token
                let token = self
                    .fetch_client_credentials_token(
                        token_url,
                        client_id,
                        client_secret,
                        scope.as_deref(),
                        extra_params,
                    )
                    .await?;

                let access_token = token.access_token.clone();
                self.token_store.store(key, token).await;

                Ok(AuthResolution::Header {
                    name: "Authorization".to_string(),
                    value: format!("Bearer {}", access_token),
                })
            }

            AuthSpec::OAuth2AuthorizationCode { .. } => {
                let key = Self::auth_spec_key(spec);

                // Check if we have a valid token
                if let Some(token) = self.token_store.get_valid(&key).await {
                    return Ok(AuthResolution::Header {
                        name: "Authorization".to_string(),
                        value: format!("Bearer {}", token.access_token),
                    });
                }

                // Check if we need to refresh
                if self.token_store.needs_refresh(&key).await {
                    if let Some(token) = self.token_store.get(&key).await {
                        if let Ok(new_token) = self.refresh_token(&token).await {
                            let access_token = new_token.access_token.clone();
                            self.token_store.store(key, new_token).await;
                            return Ok(AuthResolution::Header {
                                name: "Authorization".to_string(),
                                value: format!("Bearer {}", access_token),
                            });
                        }
                    }
                }

                // Need to start authorization flow
                Ok(AuthResolution::Pending {
                    message: "Authorization required. Click 'Authorize' to continue.".to_string(),
                })
            }
        }
    }

    async fn start_authorization(&self, spec: &AuthSpec) -> Result<AuthorizationState, AuthError> {
        match spec {
            AuthSpec::OAuth2AuthorizationCode {
                auth_url,
                token_url: _,
                client_id,
                client_secret: _,
                redirect_uri,
                scope,
                extra_params: _,
            } => {
                let client = BasicClient::new(
                    ClientId::new(client_id.to_string()),
                    None,
                    AuthUrl::new(auth_url.to_string()).map_err(|e| {
                        AuthError::InvalidConfiguration {
                            message: e.to_string(),
                        }
                    })?,
                    None,
                )
                .set_redirect_uri(RedirectUrl::new(redirect_uri.to_string()).map_err(
                    |e| AuthError::InvalidConfiguration {
                        message: e.to_string(),
                    },
                )?);

                let mut auth_request = client.authorize_url(CsrfToken::new_random);

                // Add scopes
                if let Some(scope_str) = scope {
                    for s in scope_str.split_whitespace() {
                        auth_request = auth_request.add_scope(Scope::new(s.to_string()));
                    }
                }

                let (auth_url, csrf_state) = auth_request.url();

                let state = AuthorizationState {
                    auth_url: auth_url.to_string(),
                    state: csrf_state.secret().clone(),
                    pkce_verifier: None,
                    redirect_uri: redirect_uri.to_string(),
                    auth_spec_key: Self::auth_spec_key(spec),
                };

                // Store pending authorization
                self.pending_authorizations
                    .write()
                    .await
                    .insert(csrf_state.secret().clone(), state.clone());

                self.emit_event(crate::application::auth::AuthEvent::AuthorizationUrlReady {
                    url: state.auth_url.clone(),
                });

                Ok(state)
            }
            _ => Err(AuthError::InvalidConfiguration {
                message: "start_authorization only works with OAuth2 Authorization Code flow"
                    .to_string(),
            }),
        }
    }

    async fn handle_callback(
        &self,
        state: &AuthorizationState,
        code: &str,
    ) -> Result<OAuth2Token, AuthError> {
        self.emit_event(crate::application::auth::AuthEvent::ExchangingCode);

        // This is a simplified implementation - in practice you'd need to
        // reconstruct the client and exchange the code for a token
        // The actual implementation would use the pending_authorizations
        // to retrieve the full auth spec and complete the exchange

        Err(AuthError::OAuth2AuthorizationFailed {
            message: "Full implementation requires auth spec lookup".to_string(),
        })
    }

    async fn refresh_token(&self, token: &OAuth2Token) -> Result<OAuth2Token, AuthError> {
        let refresh_token = token.refresh_token.as_ref().ok_or(AuthError::TokenExpiredNoRefresh)?;

        // This is a simplified implementation - you'd need the original
        // auth spec to know the token URL
        Err(AuthError::RefreshFailed {
            message: "Token refresh requires original auth configuration".to_string(),
        })
    }
}
```

### File: `crates/infrastructure/src/auth/callback_server.rs`

```rust
//! Local HTTP server for OAuth2 authorization code callback.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};

/// OAuth2 callback server that listens for authorization codes
pub struct CallbackServer {
    port: u16,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

/// Result of an OAuth2 callback
#[derive(Debug, Clone)]
pub struct CallbackResult {
    pub code: String,
    pub state: String,
}

/// Error from callback handling
#[derive(Debug, Clone, thiserror::Error)]
pub enum CallbackError {
    #[error("Failed to bind to port {port}: {message}")]
    BindError { port: u16, message: String },

    #[error("Callback timeout")]
    Timeout,

    #[error("User denied authorization: {message}")]
    UserDenied { message: String },

    #[error("Missing authorization code")]
    MissingCode,

    #[error("State mismatch: expected {expected}, got {actual}")]
    StateMismatch { expected: String, actual: String },

    #[error("Server error: {message}")]
    ServerError { message: String },
}

impl CallbackServer {
    /// Create a new callback server on the specified port
    pub fn new(port: u16) -> Self {
        Self {
            port,
            shutdown_tx: None,
        }
    }

    /// Start the server and wait for a callback
    ///
    /// Returns the authorization code and state, or an error.
    pub async fn wait_for_callback(
        &mut self,
        expected_state: &str,
        timeout: std::time::Duration,
    ) -> Result<CallbackResult, CallbackError> {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));

        let listener = TcpListener::bind(addr).await.map_err(|e| CallbackError::BindError {
            port: self.port,
            message: e.to_string(),
        })?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        let result = Arc::new(Mutex::new(None::<Result<CallbackResult, CallbackError>>));
        let result_clone = result.clone();
        let expected_state = expected_state.to_string();

        let server = async move {
            loop {
                tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, _)) => {
                                let result = Self::handle_connection(
                                    stream,
                                    &expected_state
                                ).await;
                                *result_clone.lock().await = Some(result);
                                break;
                            }
                            Err(e) => {
                                *result_clone.lock().await = Some(Err(CallbackError::ServerError {
                                    message: e.to_string(),
                                }));
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx => {
                        break;
                    }
                }
            }
        };

        // Run with timeout
        match tokio::time::timeout(timeout, server).await {
            Ok(_) => {
                result
                    .lock()
                    .await
                    .take()
                    .unwrap_or(Err(CallbackError::ServerError {
                        message: "No result received".to_string(),
                    }))
            }
            Err(_) => Err(CallbackError::Timeout),
        }
    }

    async fn handle_connection(
        mut stream: tokio::net::TcpStream,
        expected_state: &str,
    ) -> Result<CallbackResult, CallbackError> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let mut reader = BufReader::new(&mut stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).await.map_err(|e| {
            CallbackError::ServerError {
                message: e.to_string(),
            }
        })?;

        // Parse the request line: GET /callback?code=xxx&state=yyy HTTP/1.1
        let path = request_line
            .split_whitespace()
            .nth(1)
            .ok_or(CallbackError::ServerError {
                message: "Invalid request".to_string(),
            })?;

        // Parse query parameters
        let url = url::Url::parse(&format!("http://localhost{}", path)).map_err(|e| {
            CallbackError::ServerError {
                message: e.to_string(),
            }
        })?;

        let params: std::collections::HashMap<_, _> = url.query_pairs().collect();

        // Check for error response
        if let Some(error) = params.get("error") {
            let description = params
                .get("error_description")
                .map(|s| s.to_string())
                .unwrap_or_else(|| error.to_string());

            // Send error response to browser
            let response = Self::html_response(
                "Authorization Failed",
                &format!("<p>Error: {}</p><p>You can close this window.</p>", description),
            );
            stream.write_all(response.as_bytes()).await.ok();

            return Err(CallbackError::UserDenied { message: description });
        }

        let code = params
            .get("code")
            .ok_or(CallbackError::MissingCode)?
            .to_string();

        let state = params
            .get("state")
            .map(|s| s.to_string())
            .unwrap_or_default();

        // Verify state
        if state != expected_state {
            let response = Self::html_response(
                "Authorization Failed",
                "<p>State mismatch - possible CSRF attack.</p><p>You can close this window.</p>",
            );
            stream.write_all(response.as_bytes()).await.ok();

            return Err(CallbackError::StateMismatch {
                expected: expected_state.to_string(),
                actual: state,
            });
        }

        // Send success response
        let response = Self::html_response(
            "Authorization Successful",
            "<p>Authorization complete!</p><p>You can close this window and return to Vortex.</p>",
        );
        stream.write_all(response.as_bytes()).await.ok();

        Ok(CallbackResult { code, state })
    }

    fn html_response(title: &str, body: &str) -> String {
        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>{}</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: #1e1e1e;
            color: #cccccc;
        }}
        .container {{
            text-align: center;
            padding: 40px;
            background: #252526;
            border-radius: 8px;
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.3);
        }}
        h1 {{ color: #4fc1ff; margin-bottom: 20px; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>{}</h1>
        {}
    </div>
</body>
</html>"#,
            title, title, body
        );

        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        )
    }

    /// Shutdown the server
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for CallbackServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}
```

### File: `crates/infrastructure/src/http/body_builder.rs`

```rust
//! Request body builder for reqwest.

use crate::domain::body::{FormDataField, GraphQLRequest, RequestBody};
use reqwest::multipart;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur when building request bodies
#[derive(Debug, Error)]
pub enum BodyBuildError {
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Failed to read file: {path}: {message}")]
    FileReadError { path: String, message: String },

    #[error("JSON serialization error: {message}")]
    JsonError { message: String },

    #[error("Form encoding error: {message}")]
    FormError { message: String },
}

/// Result of building a body
pub enum BuiltBody {
    /// No body
    None,
    /// Text/JSON body with content type
    Text {
        content: String,
        content_type: &'static str,
    },
    /// Binary body
    Binary {
        data: Vec<u8>,
        content_type: String,
    },
    /// Multipart form
    Multipart(multipart::Form),
}

/// Builder for request bodies
pub struct BodyBuilder {
    workspace_root: Option<std::path::PathBuf>,
}

impl BodyBuilder {
    pub fn new() -> Self {
        Self {
            workspace_root: None,
        }
    }

    /// Set the workspace root for resolving relative file paths
    pub fn with_workspace_root(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.workspace_root = Some(root.into());
        self
    }

    /// Build a reqwest-compatible body from a RequestBody
    pub async fn build(&self, body: &RequestBody) -> Result<BuiltBody, BodyBuildError> {
        match body {
            RequestBody::None => Ok(BuiltBody::None),

            RequestBody::Json { content } => {
                let json = serde_json::to_string_pretty(content).map_err(|e| {
                    BodyBuildError::JsonError {
                        message: e.to_string(),
                    }
                })?;
                Ok(BuiltBody::Text {
                    content: json,
                    content_type: "application/json",
                })
            }

            RequestBody::Text { content } => Ok(BuiltBody::Text {
                content: content.clone(),
                content_type: "text/plain",
            }),

            RequestBody::FormUrlencoded { fields } => {
                let encoded = serde_urlencoded::to_string(fields).map_err(|e| {
                    BodyBuildError::FormError {
                        message: e.to_string(),
                    }
                })?;
                Ok(BuiltBody::Text {
                    content: encoded,
                    content_type: "application/x-www-form-urlencoded",
                })
            }

            RequestBody::FormData { fields } => {
                let form = self.build_multipart(fields).await?;
                Ok(BuiltBody::Multipart(form))
            }

            RequestBody::Binary { path, content_type } => {
                let resolved_path = self.resolve_path(path);
                let data = tokio::fs::read(&resolved_path).await.map_err(|e| {
                    BodyBuildError::FileReadError {
                        path: resolved_path.display().to_string(),
                        message: e.to_string(),
                    }
                })?;

                let mime = content_type
                    .clone()
                    .unwrap_or_else(|| {
                        mime_guess::from_path(&resolved_path)
                            .first_or_octet_stream()
                            .to_string()
                    });

                Ok(BuiltBody::Binary {
                    data,
                    content_type: mime,
                })
            }

            RequestBody::GraphQL {
                query,
                variables,
                operation_name,
            } => {
                let request = GraphQLRequest {
                    query: query.clone(),
                    variables: variables.clone(),
                    operation_name: operation_name.clone(),
                };
                let json = serde_json::to_string(&request).map_err(|e| {
                    BodyBuildError::JsonError {
                        message: e.to_string(),
                    }
                })?;
                Ok(BuiltBody::Text {
                    content: json,
                    content_type: "application/json",
                })
            }
        }
    }

    async fn build_multipart(
        &self,
        fields: &[FormDataField],
    ) -> Result<multipart::Form, BodyBuildError> {
        let mut form = multipart::Form::new();

        for field in fields {
            match field {
                FormDataField::Text { name, value } => {
                    form = form.text(name.clone(), value.clone());
                }
                FormDataField::File {
                    name,
                    path,
                    filename,
                    content_type,
                } => {
                    let resolved_path = self.resolve_path(path);

                    // Check file exists
                    if !resolved_path.exists() {
                        return Err(BodyBuildError::FileNotFound {
                            path: resolved_path.display().to_string(),
                        });
                    }

                    let data = tokio::fs::read(&resolved_path).await.map_err(|e| {
                        BodyBuildError::FileReadError {
                            path: resolved_path.display().to_string(),
                            message: e.to_string(),
                        }
                    })?;

                    let file_name = filename
                        .clone()
                        .or_else(|| {
                            resolved_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.to_string())
                        })
                        .unwrap_or_else(|| "file".to_string());

                    let mime = content_type
                        .clone()
                        .unwrap_or_else(|| {
                            mime_guess::from_path(&resolved_path)
                                .first_or_octet_stream()
                                .to_string()
                        });

                    let part = multipart::Part::bytes(data)
                        .file_name(file_name)
                        .mime_str(&mime)
                        .map_err(|e| BodyBuildError::FormError {
                            message: e.to_string(),
                        })?;

                    form = form.part(name.clone(), part);
                }
            }
        }

        Ok(form)
    }

    fn resolve_path(&self, path: &Path) -> std::path::PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(ref root) = self.workspace_root {
            root.join(path)
        } else {
            path.to_path_buf()
        }
    }
}

impl Default for BodyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for applying BuiltBody to reqwest::RequestBuilder
pub trait RequestBuilderExt {
    fn with_body(self, body: BuiltBody) -> Self;
}

impl RequestBuilderExt for reqwest::RequestBuilder {
    fn with_body(self, body: BuiltBody) -> Self {
        match body {
            BuiltBody::None => self,
            BuiltBody::Text {
                content,
                content_type,
            } => self.header("Content-Type", content_type).body(content),
            BuiltBody::Binary { data, content_type } => {
                self.header("Content-Type", content_type).body(data)
            }
            BuiltBody::Multipart(form) => self.multipart(form),
        }
    }
}
```

### File: `crates/infrastructure/src/http/tls_config_builder.rs`

```rust
//! TLS configuration builder for reqwest client.

use crate::domain::tls::{
    CertificateInfo, CertificateSource, ClientCertificate, PrivateKeySource, TlsConfig,
    TlsVersion,
};
use reqwest::{Certificate, Identity};
use std::path::Path;
use thiserror::Error;

/// Errors that can occur when building TLS configuration
#[derive(Debug, Error)]
pub enum TlsConfigError {
    #[error("Failed to read certificate file: {path}: {message}")]
    CertificateReadError { path: String, message: String },

    #[error("Failed to parse certificate: {message}")]
    CertificateParseError { message: String },

    #[error("Failed to read private key: {path}: {message}")]
    PrivateKeyReadError { path: String, message: String },

    #[error("Failed to parse private key: {message}")]
    PrivateKeyParseError { message: String },

    #[error("Failed to create identity: {message}")]
    IdentityError { message: String },

    #[error("Unsupported TLS version: {version:?}")]
    UnsupportedTlsVersion { version: TlsVersion },
}

/// Builder for applying TLS configuration to reqwest client
pub struct TlsConfigBuilder {
    config: TlsConfig,
}

impl TlsConfigBuilder {
    pub fn new(config: TlsConfig) -> Self {
        Self { config }
    }

    /// Apply TLS configuration to a reqwest ClientBuilder
    pub async fn apply(
        &self,
        mut builder: reqwest::ClientBuilder,
    ) -> Result<reqwest::ClientBuilder, TlsConfigError> {
        // Add custom CA certificates
        for ca_source in &self.config.ca_certificates {
            let cert = self.load_certificate(ca_source).await?;
            builder = builder.add_root_certificate(cert);
        }

        // Add client certificate for mTLS
        if let Some(ref client_cert) = self.config.client_certificate {
            let identity = self.load_identity(client_cert).await?;
            builder = builder.identity(identity);
        }

        // Set minimum TLS version
        if let Some(min_version) = self.config.min_tls_version {
            builder = builder.min_tls_version(self.convert_tls_version(min_version)?);
        }

        // Handle dangerous options
        if self.config.danger_accept_invalid_certs || !self.config.verify_certificates {
            builder = builder.danger_accept_invalid_certs(true);
        }

        if self.config.danger_accept_invalid_hostnames {
            builder = builder.danger_accept_invalid_hostnames(true);
        }

        Ok(builder)
    }

    async fn load_certificate(
        &self,
        source: &CertificateSource,
    ) -> Result<Certificate, TlsConfigError> {
        match source {
            CertificateSource::PemFile { path } => {
                let data = tokio::fs::read(path).await.map_err(|e| {
                    TlsConfigError::CertificateReadError {
                        path: path.display().to_string(),
                        message: e.to_string(),
                    }
                })?;
                Certificate::from_pem(&data).map_err(|e| TlsConfigError::CertificateParseError {
                    message: e.to_string(),
                })
            }
            CertificateSource::DerFile { path } => {
                let data = tokio::fs::read(path).await.map_err(|e| {
                    TlsConfigError::CertificateReadError {
                        path: path.display().to_string(),
                        message: e.to_string(),
                    }
                })?;
                Certificate::from_der(&data).map_err(|e| TlsConfigError::CertificateParseError {
                    message: e.to_string(),
                })
            }
            CertificateSource::PemContent { content } => {
                Certificate::from_pem(content.as_bytes()).map_err(|e| {
                    TlsConfigError::CertificateParseError {
                        message: e.to_string(),
                    }
                })
            }
            CertificateSource::System => {
                // System certificates are automatically included by reqwest
                // This is a no-op, but we need to return something
                Err(TlsConfigError::CertificateParseError {
                    message: "System certificates are included by default".to_string(),
                })
            }
        }
    }

    async fn load_identity(
        &self,
        client_cert: &ClientCertificate,
    ) -> Result<Identity, TlsConfigError> {
        // For PKCS#12, we need to handle it specially
        if let PrivateKeySource::Pkcs12File { path } = &client_cert.private_key {
            let data = tokio::fs::read(path).await.map_err(|e| {
                TlsConfigError::PrivateKeyReadError {
                    path: path.display().to_string(),
                    message: e.to_string(),
                }
            })?;

            let password = client_cert.password.as_deref().unwrap_or("");
            return Identity::from_pkcs12_der(&data, password).map_err(|e| {
                TlsConfigError::IdentityError {
                    message: e.to_string(),
                }
            });
        }

        // For PEM format, combine cert and key
        let cert_pem = self.load_pem_content(&client_cert.certificate).await?;
        let key_pem = self.load_key_pem_content(&client_cert.private_key).await?;

        // Combine into a single PEM
        let combined = format!("{}\n{}", cert_pem, key_pem);

        Identity::from_pem(combined.as_bytes()).map_err(|e| TlsConfigError::IdentityError {
            message: e.to_string(),
        })
    }

    async fn load_pem_content(&self, source: &CertificateSource) -> Result<String, TlsConfigError> {
        match source {
            CertificateSource::PemFile { path } => {
                tokio::fs::read_to_string(path).await.map_err(|e| {
                    TlsConfigError::CertificateReadError {
                        path: path.display().to_string(),
                        message: e.to_string(),
                    }
                })
            }
            CertificateSource::PemContent { content } => Ok(content.clone()),
            _ => Err(TlsConfigError::CertificateParseError {
                message: "Expected PEM format for client certificate".to_string(),
            }),
        }
    }

    async fn load_key_pem_content(
        &self,
        source: &PrivateKeySource,
    ) -> Result<String, TlsConfigError> {
        match source {
            PrivateKeySource::PemFile { path } => {
                tokio::fs::read_to_string(path).await.map_err(|e| {
                    TlsConfigError::PrivateKeyReadError {
                        path: path.display().to_string(),
                        message: e.to_string(),
                    }
                })
            }
            PrivateKeySource::PemContent { content } => Ok(content.clone()),
            _ => Err(TlsConfigError::PrivateKeyParseError {
                message: "Expected PEM format for private key".to_string(),
            }),
        }
    }

    fn convert_tls_version(
        &self,
        version: TlsVersion,
    ) -> Result<reqwest::tls::Version, TlsConfigError> {
        match version {
            TlsVersion::Tls12 => Ok(reqwest::tls::Version::TLS_1_2),
            TlsVersion::Tls13 => Ok(reqwest::tls::Version::TLS_1_3),
            v => Err(TlsConfigError::UnsupportedTlsVersion { version: v }),
        }
    }
}

/// Parse a PEM certificate and extract basic information
pub fn parse_certificate_info(pem_data: &[u8]) -> Result<CertificateInfo, TlsConfigError> {
    // This is a simplified implementation. A full implementation would use
    // x509-parser or rustls-pemfile to extract certificate details.
    Err(TlsConfigError::CertificateParseError {
        message: "Certificate parsing not fully implemented".to_string(),
    })
}

/// User-friendly error messages for TLS errors
pub fn friendly_tls_error_message(error: &reqwest::Error) -> String {
    if error.is_connect() {
        if let Some(source) = error.source() {
            let msg = source.to_string().to_lowercase();

            if msg.contains("certificate") && msg.contains("expired") {
                return "The server's SSL certificate has expired. Contact the server administrator.".to_string();
            }

            if msg.contains("self-signed") || msg.contains("self signed") {
                return "The server is using a self-signed certificate. \
                        You can add it to trusted certificates or enable 'Accept invalid certificates' (not recommended for production).".to_string();
            }

            if msg.contains("hostname") || msg.contains("name") {
                return "The server's certificate doesn't match the hostname. \
                        Verify you're connecting to the correct server.".to_string();
            }

            if msg.contains("unknown ca") || msg.contains("unable to get local issuer") {
                return "The server's certificate is signed by an unknown CA. \
                        You may need to add a custom CA certificate.".to_string();
            }
        }
    }

    error.to_string()
}
```

---

## Part 4: UI Components (Slint)

### File: `crates/ui/src/components/auth_panel.slint`

```slint
// Authentication panel component for request configuration

import { VerticalBox, HorizontalBox, ComboBox, LineEdit, Button, CheckBox } from "std-widgets.slint";

// Auth type enumeration
export enum AuthType {
    None,
    Bearer,
    Basic,
    ApiKey,
    OAuth2ClientCredentials,
    OAuth2AuthorizationCode,
}

// Token status for OAuth2
export enum TokenStatus {
    NotAuthenticated,
    Valid,
    ExpiringSoon,
    Expired,
    Refreshing,
    Error,
}

// API Key location
export enum ApiKeyLocation {
    Header,
    Query,
}

export component AuthPanel inherits VerticalBox {
    // Current auth type
    in-out property <AuthType> auth-type: AuthType.None;

    // Bearer token properties
    in-out property <string> bearer-token;
    in-out property <string> bearer-prefix: "Bearer";

    // Basic auth properties
    in-out property <string> basic-username;
    in-out property <string> basic-password;

    // API Key properties
    in-out property <string> api-key-name;
    in-out property <string> api-key-value;
    in-out property <ApiKeyLocation> api-key-location: ApiKeyLocation.Header;

    // OAuth2 Client Credentials
    in-out property <string> oauth2-cc-token-url;
    in-out property <string> oauth2-cc-client-id;
    in-out property <string> oauth2-cc-client-secret;
    in-out property <string> oauth2-cc-scope;

    // OAuth2 Auth Code
    in-out property <string> oauth2-ac-auth-url;
    in-out property <string> oauth2-ac-token-url;
    in-out property <string> oauth2-ac-client-id;
    in-out property <string> oauth2-ac-client-secret;
    in-out property <string> oauth2-ac-redirect-uri: "http://localhost:9876/callback";
    in-out property <string> oauth2-ac-scope;

    // Token status
    in-out property <TokenStatus> token-status: TokenStatus.NotAuthenticated;
    in-out property <string> token-status-message;
    in-out property <int> token-expires-in-seconds;

    // Callbacks
    callback auth-type-changed(AuthType);
    callback authorize-clicked();
    callback refresh-token-clicked();
    callback clear-token-clicked();

    padding: 16px;
    spacing: 12px;

    // Auth type selector
    HorizontalBox {
        spacing: 8px;
        Text {
            text: "Type:";
            vertical-alignment: center;
        }
        ComboBox {
            model: ["No Auth", "Bearer Token", "Basic Auth", "API Key", "OAuth2 (Client Credentials)", "OAuth2 (Authorization Code)"];
            current-index: root.auth-type == AuthType.None ? 0 :
                          root.auth-type == AuthType.Bearer ? 1 :
                          root.auth-type == AuthType.Basic ? 2 :
                          root.auth-type == AuthType.ApiKey ? 3 :
                          root.auth-type == AuthType.OAuth2ClientCredentials ? 4 : 5;
            selected(index) => {
                if (index == 0) { root.auth-type = AuthType.None; }
                else if (index == 1) { root.auth-type = AuthType.Bearer; }
                else if (index == 2) { root.auth-type = AuthType.Basic; }
                else if (index == 3) { root.auth-type = AuthType.ApiKey; }
                else if (index == 4) { root.auth-type = AuthType.OAuth2ClientCredentials; }
                else { root.auth-type = AuthType.OAuth2AuthorizationCode; }
                root.auth-type-changed(root.auth-type);
            }
        }
    }

    // Dynamic content based on auth type
    if root.auth-type == AuthType.None : Rectangle {
        Text {
            text: "This request does not use any authorization.";
            color: #858585;
        }
    }

    if root.auth-type == AuthType.Bearer : VerticalBox {
        spacing: 8px;
        HorizontalBox {
            spacing: 8px;
            Text { text: "Token:"; vertical-alignment: center; min-width: 80px; }
            LineEdit {
                text <=> root.bearer-token;
                placeholder-text: "{{access_token}}";
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Prefix:"; vertical-alignment: center; min-width: 80px; }
            LineEdit {
                text <=> root.bearer-prefix;
                placeholder-text: "Bearer";
                max-width: 150px;
            }
        }
        Text {
            text: "Variables like {{token}} are supported";
            color: #858585;
            font-size: 11px;
        }
    }

    if root.auth-type == AuthType.Basic : VerticalBox {
        spacing: 8px;
        HorizontalBox {
            spacing: 8px;
            Text { text: "Username:"; vertical-alignment: center; min-width: 80px; }
            LineEdit {
                text <=> root.basic-username;
                placeholder-text: "username";
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Password:"; vertical-alignment: center; min-width: 80px; }
            LineEdit {
                text <=> root.basic-password;
                placeholder-text: "password";
                input-type: password;
                horizontal-stretch: 1;
            }
        }
    }

    if root.auth-type == AuthType.ApiKey : VerticalBox {
        spacing: 8px;
        HorizontalBox {
            spacing: 8px;
            Text { text: "Key:"; vertical-alignment: center; min-width: 80px; }
            LineEdit {
                text <=> root.api-key-name;
                placeholder-text: "X-API-Key";
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Value:"; vertical-alignment: center; min-width: 80px; }
            LineEdit {
                text <=> root.api-key-value;
                placeholder-text: "{{api_key}}";
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Add to:"; vertical-alignment: center; min-width: 80px; }
            ComboBox {
                model: ["Header", "Query Params"];
                current-index: root.api-key-location == ApiKeyLocation.Header ? 0 : 1;
                selected(index) => {
                    root.api-key-location = index == 0 ? ApiKeyLocation.Header : ApiKeyLocation.Query;
                }
            }
        }
    }

    if root.auth-type == AuthType.OAuth2ClientCredentials : VerticalBox {
        spacing: 8px;
        HorizontalBox {
            spacing: 8px;
            Text { text: "Token URL:"; vertical-alignment: center; min-width: 100px; }
            LineEdit {
                text <=> root.oauth2-cc-token-url;
                placeholder-text: "https://auth.example.com/oauth/token";
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Client ID:"; vertical-alignment: center; min-width: 100px; }
            LineEdit {
                text <=> root.oauth2-cc-client-id;
                placeholder-text: "your-client-id";
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Client Secret:"; vertical-alignment: center; min-width: 100px; }
            LineEdit {
                text <=> root.oauth2-cc-client-secret;
                placeholder-text: "your-client-secret";
                input-type: password;
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Scope:"; vertical-alignment: center; min-width: 100px; }
            LineEdit {
                text <=> root.oauth2-cc-scope;
                placeholder-text: "read write (space-separated)";
                horizontal-stretch: 1;
            }
        }

        // Token status display
        token-status-bar := HorizontalBox {
            spacing: 8px;
            padding-top: 8px;

            Rectangle {
                width: 12px;
                height: 12px;
                border-radius: 6px;
                background: root.token-status == TokenStatus.Valid ? #4ec9b0 :
                           root.token-status == TokenStatus.ExpiringSoon ? #ce9178 :
                           root.token-status == TokenStatus.Refreshing ? #569cd6 :
                           root.token-status == TokenStatus.Error ? #f14c4c : #858585;
            }

            Text {
                text: root.token-status-message;
                color: root.token-status == TokenStatus.Error ? #f14c4c : #cccccc;
                horizontal-stretch: 1;
            }

            if root.token-status == TokenStatus.Valid || root.token-status == TokenStatus.ExpiringSoon : Button {
                text: "Refresh";
                clicked => { root.refresh-token-clicked(); }
            }

            if root.token-status != TokenStatus.NotAuthenticated : Button {
                text: "Clear";
                clicked => { root.clear-token-clicked(); }
            }
        }
    }

    if root.auth-type == AuthType.OAuth2AuthorizationCode : VerticalBox {
        spacing: 8px;
        HorizontalBox {
            spacing: 8px;
            Text { text: "Auth URL:"; vertical-alignment: center; min-width: 100px; }
            LineEdit {
                text <=> root.oauth2-ac-auth-url;
                placeholder-text: "https://auth.example.com/oauth/authorize";
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Token URL:"; vertical-alignment: center; min-width: 100px; }
            LineEdit {
                text <=> root.oauth2-ac-token-url;
                placeholder-text: "https://auth.example.com/oauth/token";
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Client ID:"; vertical-alignment: center; min-width: 100px; }
            LineEdit {
                text <=> root.oauth2-ac-client-id;
                placeholder-text: "your-client-id";
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Client Secret:"; vertical-alignment: center; min-width: 100px; }
            LineEdit {
                text <=> root.oauth2-ac-client-secret;
                input-type: password;
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Redirect URI:"; vertical-alignment: center; min-width: 100px; }
            LineEdit {
                text <=> root.oauth2-ac-redirect-uri;
                placeholder-text: "http://localhost:9876/callback";
                horizontal-stretch: 1;
            }
        }
        HorizontalBox {
            spacing: 8px;
            Text { text: "Scope:"; vertical-alignment: center; min-width: 100px; }
            LineEdit {
                text <=> root.oauth2-ac-scope;
                placeholder-text: "openid profile email";
                horizontal-stretch: 1;
            }
        }

        // Authorization button and status
        HorizontalBox {
            spacing: 8px;
            padding-top: 12px;

            Button {
                text: root.token-status == TokenStatus.NotAuthenticated ? "Authorize" : "Re-authorize";
                primary: true;
                clicked => { root.authorize-clicked(); }
            }

            Rectangle {
                width: 12px;
                height: 12px;
                border-radius: 6px;
                background: root.token-status == TokenStatus.Valid ? #4ec9b0 :
                           root.token-status == TokenStatus.ExpiringSoon ? #ce9178 :
                           root.token-status == TokenStatus.Refreshing ? #569cd6 :
                           root.token-status == TokenStatus.Error ? #f14c4c : #858585;
                visible: root.token-status != TokenStatus.NotAuthenticated;
            }

            Text {
                text: root.token-status-message;
                color: root.token-status == TokenStatus.Error ? #f14c4c : #cccccc;
                horizontal-stretch: 1;
                visible: root.token-status != TokenStatus.NotAuthenticated;
            }
        }
    }
}
```

### File: `crates/ui/src/components/body_editor.slint`

```slint
// Body editor component with tabs for different body types

import { VerticalBox, HorizontalBox, TabWidget, TextEdit, LineEdit, Button, ComboBox } from "std-widgets.slint";

export enum BodyType {
    None,
    Json,
    Text,
    FormUrlencoded,
    FormData,
    Binary,
    GraphQL,
}

export struct FormField {
    enabled: bool,
    key: string,
    value: string,
    description: string,
}

export struct FormDataFieldItem {
    enabled: bool,
    name: string,
    field-type: string, // "text" or "file"
    value: string,      // text value or file path
    content-type: string,
}

export component BodyEditor inherits VerticalBox {
    in-out property <BodyType> body-type: BodyType.None;

    // JSON/Text body
    in-out property <string> raw-content;
    in-out property <bool> json-valid: true;
    in-out property <string> json-error;

    // Form URL encoded
    in-out property <[FormField]> form-fields: [];

    // Form data (multipart)
    in-out property <[FormDataFieldItem]> form-data-fields: [];

    // Binary
    in-out property <string> binary-file-path;
    in-out property <string> binary-content-type;

    // GraphQL
    in-out property <string> graphql-query;
    in-out property <string> graphql-variables;
    in-out property <string> graphql-operation-name;

    // Callbacks
    callback body-type-changed(BodyType);
    callback content-changed(string);
    callback add-form-field();
    callback remove-form-field(int);
    callback add-form-data-field(string); // "text" or "file"
    callback remove-form-data-field(int);
    callback browse-file-clicked();
    callback format-json-clicked();
    callback validate-json();

    padding: 0px;
    spacing: 0px;

    // Body type tabs
    type-tabs := HorizontalBox {
        spacing: 0px;
        padding: 8px;

        for type-info in [
            { label: "none", type: BodyType.None },
            { label: "json", type: BodyType.Json },
            { label: "text", type: BodyType.Text },
            { label: "x-www-form-urlencoded", type: BodyType.FormUrlencoded },
            { label: "form-data", type: BodyType.FormData },
            { label: "binary", type: BodyType.Binary },
            { label: "GraphQL", type: BodyType.GraphQL },
        ] : Rectangle {
            padding-left: 12px;
            padding-right: 12px;
            padding-top: 6px;
            padding-bottom: 6px;
            border-radius: 4px;
            background: root.body-type == type-info.type ? #094771 : transparent;

            Text {
                text: type-info.label;
                color: root.body-type == type-info.type ? #ffffff : #cccccc;
                font-size: 12px;
            }

            TouchArea {
                clicked => {
                    root.body-type = type-info.type;
                    root.body-type-changed(type-info.type);
                }
            }
        }
    }

    // Content area
    Rectangle {
        background: #1e1e1e;
        horizontal-stretch: 1;
        vertical-stretch: 1;

        if root.body-type == BodyType.None : VerticalBox {
            padding: 20px;
            Text {
                text: "This request does not have a body.";
                color: #858585;
                horizontal-alignment: center;
            }
        }

        if root.body-type == BodyType.Json || root.body-type == BodyType.Text : VerticalBox {
            spacing: 0px;

            // Toolbar
            HorizontalBox {
                spacing: 8px;
                padding: 8px;

                if root.body-type == BodyType.Json : Button {
                    text: "Format";
                    clicked => { root.format-json-clicked(); }
                }

                if root.body-type == BodyType.Json && !root.json-valid : Text {
                    text: root.json-error;
                    color: #f14c4c;
                    font-size: 11px;
                    vertical-alignment: center;
                }

                Rectangle { horizontal-stretch: 1; }
            }

            // Editor
            TextEdit {
                text <=> root.raw-content;
                font-family: "JetBrains Mono";
                font-size: 13px;
                horizontal-stretch: 1;
                vertical-stretch: 1;

                edited(text) => {
                    root.content-changed(text);
                    if (root.body-type == BodyType.Json) {
                        root.validate-json();
                    }
                }
            }
        }

        if root.body-type == BodyType.FormUrlencoded : VerticalBox {
            spacing: 0px;

            // Header row
            HorizontalBox {
                padding: 8px;
                spacing: 8px;
                background: #252526;

                Text { text: ""; width: 24px; } // Checkbox column
                Text { text: "Key"; min-width: 150px; horizontal-stretch: 1; }
                Text { text: "Value"; min-width: 150px; horizontal-stretch: 1; }
                Text { text: ""; width: 32px; } // Delete column
            }

            // Fields list
            for field[index] in root.form-fields : HorizontalBox {
                padding: 4px;
                padding-left: 8px;
                padding-right: 8px;
                spacing: 8px;

                CheckBox {
                    checked: field.enabled;
                    width: 24px;
                }

                LineEdit {
                    text: field.key;
                    placeholder-text: "key";
                    min-width: 150px;
                    horizontal-stretch: 1;
                }

                LineEdit {
                    text: field.value;
                    placeholder-text: "value";
                    min-width: 150px;
                    horizontal-stretch: 1;
                }

                Button {
                    text: "x";
                    width: 32px;
                    clicked => { root.remove-form-field(index); }
                }
            }

            // Add button
            HorizontalBox {
                padding: 8px;
                Button {
                    text: "+ Add Parameter";
                    clicked => { root.add-form-field(); }
                }
            }

            Rectangle { vertical-stretch: 1; }
        }

        if root.body-type == BodyType.FormData : VerticalBox {
            spacing: 0px;

            // Header row
            HorizontalBox {
                padding: 8px;
                spacing: 8px;
                background: #252526;

                Text { text: ""; width: 24px; }
                Text { text: "Key"; min-width: 120px; horizontal-stretch: 1; }
                Text { text: "Type"; width: 80px; }
                Text { text: "Value"; min-width: 150px; horizontal-stretch: 1; }
                Text { text: ""; width: 32px; }
            }

            // Fields list
            for field[index] in root.form-data-fields : HorizontalBox {
                padding: 4px;
                padding-left: 8px;
                padding-right: 8px;
                spacing: 8px;

                CheckBox {
                    checked: field.enabled;
                    width: 24px;
                }

                LineEdit {
                    text: field.name;
                    placeholder-text: "name";
                    min-width: 120px;
                    horizontal-stretch: 1;
                }

                ComboBox {
                    model: ["Text", "File"];
                    current-index: field.field-type == "text" ? 0 : 1;
                    width: 80px;
                }

                if field.field-type == "text" : LineEdit {
                    text: field.value;
                    placeholder-text: "value";
                    min-width: 150px;
                    horizontal-stretch: 1;
                }

                if field.field-type == "file" : HorizontalBox {
                    spacing: 4px;
                    min-width: 150px;
                    horizontal-stretch: 1;

                    LineEdit {
                        text: field.value;
                        placeholder-text: "Select a file...";
                        horizontal-stretch: 1;
                    }
                    Button {
                        text: "...";
                        width: 32px;
                        clicked => { root.browse-file-clicked(); }
                    }
                }

                Button {
                    text: "x";
                    width: 32px;
                    clicked => { root.remove-form-data-field(index); }
                }
            }

            // Add buttons
            HorizontalBox {
                padding: 8px;
                spacing: 8px;

                Button {
                    text: "+ Add Text";
                    clicked => { root.add-form-data-field("text"); }
                }
                Button {
                    text: "+ Add File";
                    clicked => { root.add-form-data-field("file"); }
                }
            }

            Rectangle { vertical-stretch: 1; }
        }

        if root.body-type == BodyType.Binary : VerticalBox {
            padding: 16px;
            spacing: 12px;

            Text {
                text: "Select a file to send as the request body";
                color: #cccccc;
            }

            HorizontalBox {
                spacing: 8px;

                LineEdit {
                    text <=> root.binary-file-path;
                    placeholder-text: "Select a file...";
                    horizontal-stretch: 1;
                }

                Button {
                    text: "Browse...";
                    clicked => { root.browse-file-clicked(); }
                }
            }

            HorizontalBox {
                spacing: 8px;

                Text {
                    text: "Content-Type:";
                    vertical-alignment: center;
                }

                LineEdit {
                    text <=> root.binary-content-type;
                    placeholder-text: "auto-detect from file";
                    horizontal-stretch: 1;
                }
            }

            Rectangle { vertical-stretch: 1; }
        }

        if root.body-type == BodyType.GraphQL : VerticalBox {
            spacing: 0px;

            // Query editor
            Text {
                text: "Query";
                padding: 8px;
                font-weight: 600;
            }

            TextEdit {
                text <=> root.graphql-query;
                font-family: "JetBrains Mono";
                font-size: 13px;
                horizontal-stretch: 1;
                min-height: 150px;

                // placeholder-text: "query GetUser($id: ID!) {\n  user(id: $id) {\n    name\n    email\n  }\n}";
            }

            // Variables editor
            Text {
                text: "Variables (JSON)";
                padding: 8px;
                font-weight: 600;
            }

            TextEdit {
                text <=> root.graphql-variables;
                font-family: "JetBrains Mono";
                font-size: 13px;
                horizontal-stretch: 1;
                min-height: 80px;

                // placeholder-text: "{\n  \"id\": \"123\"\n}";
            }

            // Operation name (optional)
            HorizontalBox {
                padding: 8px;
                spacing: 8px;

                Text {
                    text: "Operation Name:";
                    vertical-alignment: center;
                }

                LineEdit {
                    text <=> root.graphql-operation-name;
                    placeholder-text: "(optional)";
                    horizontal-stretch: 1;
                }
            }
        }
    }
}
```

### File: `crates/ui/src/components/tls_settings.slint`

```slint
// TLS settings panel for certificate and security configuration

import { VerticalBox, HorizontalBox, CheckBox, LineEdit, Button, GroupBox } from "std-widgets.slint";

export struct CertificateItem {
    name: string,
    path: string,
    valid: bool,
    expires: string,
}

export component TlsSettingsPanel inherits VerticalBox {
    // Properties
    in-out property <bool> verify-certificates: true;
    in-out property <bool> accept-invalid-certs: false;
    in-out property <bool> accept-invalid-hostnames: false;
    in-out property <[CertificateItem]> ca-certificates: [];

    // Client certificate (mTLS)
    in-out property <bool> use-client-cert: false;
    in-out property <string> client-cert-path;
    in-out property <string> client-key-path;
    in-out property <string> client-cert-password;

    // Warnings
    in-out property <bool> show-security-warning: false;
    in-out property <string> security-warning-message;

    // Callbacks
    callback add-ca-certificate();
    callback remove-ca-certificate(int);
    callback browse-client-cert();
    callback browse-client-key();
    callback settings-changed();

    padding: 16px;
    spacing: 16px;

    // Security warning banner
    if root.show-security-warning : Rectangle {
        background: #f14c4c20;
        border-color: #f14c4c;
        border-width: 1px;
        border-radius: 4px;
        padding: 12px;

        HorizontalBox {
            spacing: 8px;

            Text {
                text: "WARNING";
                color: #f14c4c;
                font-weight: 700;
            }

            Text {
                text: root.security-warning-message;
                color: #f14c4c;
                horizontal-stretch: 1;
                wrap: word-wrap;
            }
        }
    }

    // Certificate verification
    GroupBox {
        title: "Certificate Verification";

        VerticalBox {
            spacing: 8px;
            padding: 8px;

            CheckBox {
                text: "Verify server certificates";
                checked <=> root.verify-certificates;
                toggled => {
                    root.update-security-warning();
                    root.settings-changed();
                }
            }

            CheckBox {
                text: "Accept invalid certificates (DANGEROUS)";
                checked <=> root.accept-invalid-certs;
                enabled: !root.verify-certificates;
                toggled => {
                    root.update-security-warning();
                    root.settings-changed();
                }
            }

            CheckBox {
                text: "Accept invalid hostnames (DANGEROUS)";
                checked <=> root.accept-invalid-hostnames;
                toggled => {
                    root.update-security-warning();
                    root.settings-changed();
                }
            }
        }
    }

    // Custom CA Certificates
    GroupBox {
        title: "Custom CA Certificates";

        VerticalBox {
            spacing: 8px;
            padding: 8px;

            Text {
                text: "Add trusted CA certificates for self-signed or internal servers.";
                color: #858585;
                font-size: 12px;
                wrap: word-wrap;
            }

            // Certificate list
            for cert[index] in root.ca-certificates : HorizontalBox {
                spacing: 8px;
                padding: 4px;
                background: #252526;
                border-radius: 4px;

                Rectangle {
                    width: 8px;
                    height: 8px;
                    border-radius: 4px;
                    background: cert.valid ? #4ec9b0 : #f14c4c;
                }

                Text {
                    text: cert.name;
                    horizontal-stretch: 1;
                }

                Text {
                    text: cert.expires;
                    color: #858585;
                    font-size: 11px;
                }

                Button {
                    text: "x";
                    width: 24px;
                    clicked => { root.remove-ca-certificate(index); }
                }
            }

            Button {
                text: "+ Add CA Certificate";
                clicked => { root.add-ca-certificate(); }
            }
        }
    }

    // Client Certificate (mTLS)
    GroupBox {
        title: "Client Certificate (mTLS)";

        VerticalBox {
            spacing: 8px;
            padding: 8px;

            CheckBox {
                text: "Use client certificate for mutual TLS";
                checked <=> root.use-client-cert;
                toggled => { root.settings-changed(); }
            }

            if root.use-client-cert : VerticalBox {
                spacing: 8px;

                HorizontalBox {
                    spacing: 8px;

                    Text {
                        text: "Certificate:";
                        vertical-alignment: center;
                        min-width: 80px;
                    }

                    LineEdit {
                        text <=> root.client-cert-path;
                        placeholder-text: "Path to certificate file (.pem, .crt)";
                        horizontal-stretch: 1;
                    }

                    Button {
                        text: "Browse";
                        clicked => { root.browse-client-cert(); }
                    }
                }

                HorizontalBox {
                    spacing: 8px;

                    Text {
                        text: "Private Key:";
                        vertical-alignment: center;
                        min-width: 80px;
                    }

                    LineEdit {
                        text <=> root.client-key-path;
                        placeholder-text: "Path to private key file (.pem, .key)";
                        horizontal-stretch: 1;
                    }

                    Button {
                        text: "Browse";
                        clicked => { root.browse-client-key(); }
                    }
                }

                HorizontalBox {
                    spacing: 8px;

                    Text {
                        text: "Password:";
                        vertical-alignment: center;
                        min-width: 80px;
                    }

                    LineEdit {
                        text <=> root.client-cert-password;
                        placeholder-text: "(if encrypted)";
                        input-type: password;
                        horizontal-stretch: 1;
                    }
                }
            }
        }
    }

    Rectangle { vertical-stretch: 1; }

    // Helper function (called from Rust)
    function update-security-warning() {
        if (!root.verify-certificates || root.accept-invalid-certs || root.accept-invalid-hostnames) {
            root.show-security-warning = true;
            root.security-warning-message = "Insecure settings enabled. Connections may be vulnerable to interception.";
        } else {
            root.show-security-warning = false;
        }
    }
}
```

---

## Part 5: Integration and Tests

### File: `crates/application/src/auth/mod.rs`

```rust
//! Authentication module for Vortex.

pub mod provider;
pub mod token_store;

pub use provider::{AuthEvent, AuthEventListener, AuthorizationState, AuthProvider};
pub use token_store::{TokenStatus, TokenStore};
```

### File: `crates/domain/src/lib.rs`

```rust
//! Vortex domain types.

pub mod auth;
pub mod body;
pub mod tls;

pub use auth::{ApiKeyLocation, AuthError, AuthResolution, AuthSpec, OAuth2Token};
pub use body::{BodyContentType, FormDataField, GraphQLRequest, RequestBody};
pub use tls::{
    CertificateInfo, CertificateSource, ClientCertificate, PrivateKeySource, TlsConfig,
    TlsSecurityWarning, TlsVersion,
};
```

### File: `crates/domain/tests/auth_tests.rs`

```rust
//! Tests for authentication domain types.

use vortex_domain::auth::*;

#[test]
fn test_auth_spec_serialization_bearer() {
    let auth = AuthSpec::Bearer {
        token: "{{access_token}}".to_string(),
        prefix: "Bearer".to_string(),
    };

    let json = serde_json::to_string(&auth).unwrap();
    let parsed: AuthSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(auth, parsed);
}

#[test]
fn test_auth_spec_serialization_oauth2_cc() {
    let auth = AuthSpec::OAuth2ClientCredentials {
        token_url: "https://auth.example.com/token".to_string(),
        client_id: "my-client".to_string(),
        client_secret: "secret".to_string(),
        scope: Some("read write".to_string()),
        extra_params: Default::default(),
    };

    let json = serde_json::to_string(&auth).unwrap();
    assert!(json.contains("oauth2_client_credentials"));

    let parsed: AuthSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(auth, parsed);
}

#[test]
fn test_oauth2_token_expiry() {
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    let token = OAuth2Token {
        id: Uuid::new_v4(),
        access_token: "test_token".to_string(),
        token_type: "Bearer".to_string(),
        expires_at: Some(Utc::now() + Duration::seconds(30)),
        refresh_token: Some("refresh_token".to_string()),
        scopes: vec!["read".to_string()],
        obtained_at: Utc::now(),
        auth_config_key: "test".to_string(),
    };

    // Not expired yet
    assert!(!token.is_expired_or_expiring(0));

    // Will expire within 60 seconds
    assert!(token.is_expired_or_expiring(60));

    // Can refresh
    assert!(token.can_refresh());
}

#[test]
fn test_oauth2_token_no_refresh() {
    use chrono::Utc;
    use uuid::Uuid;

    let token = OAuth2Token {
        id: Uuid::new_v4(),
        access_token: "test_token".to_string(),
        token_type: "Bearer".to_string(),
        expires_at: None,
        refresh_token: None,
        scopes: vec![],
        obtained_at: Utc::now(),
        auth_config_key: "test".to_string(),
    };

    // No expiry - never expires
    assert!(!token.is_expired_or_expiring(0));
    assert!(!token.is_expired_or_expiring(3600));

    // Cannot refresh
    assert!(!token.can_refresh());
}
```

### File: `crates/domain/tests/body_tests.rs`

```rust
//! Tests for request body domain types.

use std::path::PathBuf;
use vortex_domain::body::*;

#[test]
fn test_json_body_serialization() {
    let body = RequestBody::Json {
        content: serde_json::json!({
            "name": "test",
            "value": 42
        }),
    };

    let json = serde_json::to_string(&body).unwrap();
    assert!(json.contains("\"type\":\"json\""));

    let parsed: RequestBody = serde_json::from_str(&json).unwrap();
    assert_eq!(body, parsed);
}

#[test]
fn test_form_urlencoded_serialization() {
    let mut fields = std::collections::HashMap::new();
    fields.insert("username".to_string(), "test".to_string());
    fields.insert("password".to_string(), "secret".to_string());

    let body = RequestBody::FormUrlencoded { fields };

    let json = serde_json::to_string(&body).unwrap();
    assert!(json.contains("\"type\":\"form_urlencoded\""));

    let parsed: RequestBody = serde_json::from_str(&json).unwrap();
    assert_eq!(body, parsed);
}

#[test]
fn test_form_data_serialization() {
    let body = RequestBody::FormData {
        fields: vec![
            FormDataField::Text {
                name: "description".to_string(),
                value: "My file".to_string(),
            },
            FormDataField::File {
                name: "upload".to_string(),
                path: PathBuf::from("./test.pdf"),
                filename: None,
                content_type: Some("application/pdf".to_string()),
            },
        ],
    };

    let json = serde_json::to_string(&body).unwrap();
    assert!(json.contains("\"type\":\"form_data\""));

    let parsed: RequestBody = serde_json::from_str(&json).unwrap();
    assert_eq!(body, parsed);
}

#[test]
fn test_graphql_serialization() {
    let body = RequestBody::GraphQL {
        query: "query GetUser($id: ID!) { user(id: $id) { name } }".to_string(),
        variables: Some(serde_json::json!({ "id": "123" })),
        operation_name: Some("GetUser".to_string()),
    };

    let json = serde_json::to_string(&body).unwrap();
    assert!(json.contains("\"type\":\"graphql\""));

    let parsed: RequestBody = serde_json::from_str(&json).unwrap();
    assert_eq!(body, parsed);
}

#[test]
fn test_binary_body_content_type() {
    let body = RequestBody::Binary {
        path: PathBuf::from("image.png"),
        content_type: None,
    };

    // Content type should be guessed from extension
    if let Some(BodyContentType::Binary(mime)) = body.content_type() {
        assert!(mime.contains("image/png"));
    }
}

#[test]
fn test_required_files() {
    let body = RequestBody::FormData {
        fields: vec![
            FormDataField::Text {
                name: "text".to_string(),
                value: "value".to_string(),
            },
            FormDataField::File {
                name: "file1".to_string(),
                path: PathBuf::from("./a.pdf"),
                filename: None,
                content_type: None,
            },
            FormDataField::File {
                name: "file2".to_string(),
                path: PathBuf::from("./b.png"),
                filename: None,
                content_type: None,
            },
        ],
    };

    let files = body.required_files();
    assert_eq!(files.len(), 2);
}
```

### File: `crates/application/tests/token_store_tests.rs`

```rust
//! Tests for token store.

use chrono::{Duration, Utc};
use uuid::Uuid;
use vortex_application::auth::token_store::{TokenStatus, TokenStore};
use vortex_domain::auth::OAuth2Token;

fn create_test_token(expires_in_seconds: Option<i64>, with_refresh: bool) -> OAuth2Token {
    OAuth2Token {
        id: Uuid::new_v4(),
        access_token: "test_access_token".to_string(),
        token_type: "Bearer".to_string(),
        expires_at: expires_in_seconds.map(|s| Utc::now() + Duration::seconds(s)),
        refresh_token: if with_refresh {
            Some("test_refresh_token".to_string())
        } else {
            None
        },
        scopes: vec!["read".to_string(), "write".to_string()],
        obtained_at: Utc::now(),
        auth_config_key: "test_key".to_string(),
    }
}

#[tokio::test]
async fn test_store_and_retrieve() {
    let store = TokenStore::new();
    let token = create_test_token(Some(3600), true);

    store.store("test".to_string(), token.clone()).await;

    let retrieved = store.get("test").await.unwrap();
    assert_eq!(retrieved.access_token, "test_access_token");
}

#[tokio::test]
async fn test_get_valid_returns_none_for_expired() {
    let store = TokenStore::new();
    let token = create_test_token(Some(-10), false); // Already expired

    store.store("test".to_string(), token).await;

    assert!(store.get_valid("test").await.is_none());
}

#[tokio::test]
async fn test_needs_refresh() {
    let store = TokenStore::with_refresh_buffer(60);

    // Token expiring in 30 seconds with refresh token
    let token = create_test_token(Some(30), true);
    store.store("test".to_string(), token).await;

    // Should need refresh (within 60 second buffer)
    assert!(store.needs_refresh("test").await);
}

#[tokio::test]
async fn test_needs_refresh_false_without_refresh_token() {
    let store = TokenStore::with_refresh_buffer(60);

    // Token expiring soon but no refresh token
    let token = create_test_token(Some(30), false);
    store.store("test".to_string(), token).await;

    // Cannot refresh without refresh token
    assert!(!store.needs_refresh("test").await);
}

#[tokio::test]
async fn test_token_status() {
    let store = TokenStore::with_refresh_buffer(60);

    // Not authenticated
    assert_eq!(
        store.get_status("unknown").await,
        TokenStatus::NotAuthenticated
    );

    // Valid token
    let token = create_test_token(Some(3600), true);
    store.store("valid".to_string(), token).await;

    match store.get_status("valid").await {
        TokenStatus::Valid { seconds_remaining } => {
            assert!(seconds_remaining.unwrap() > 3500);
        }
        _ => panic!("Expected Valid status"),
    }

    // Expiring soon
    let token = create_test_token(Some(30), true);
    store.store("expiring".to_string(), token).await;

    match store.get_status("expiring").await {
        TokenStatus::ExpiringSoon { seconds_remaining } => {
            assert!(seconds_remaining <= 30);
        }
        _ => panic!("Expected ExpiringSoon status"),
    }

    // Expired
    let token = create_test_token(Some(-10), true);
    store.store("expired".to_string(), token).await;

    match store.get_status("expired").await {
        TokenStatus::Expired { can_refresh } => {
            assert!(can_refresh);
        }
        _ => panic!("Expected Expired status"),
    }
}

#[tokio::test]
async fn test_clear() {
    let store = TokenStore::new();

    store
        .store("a".to_string(), create_test_token(Some(3600), false))
        .await;
    store
        .store("b".to_string(), create_test_token(Some(3600), false))
        .await;

    assert_eq!(store.keys().await.len(), 2);

    store.clear().await;

    assert_eq!(store.keys().await.len(), 0);
}
```

---

## Part 6: Implementation Order

### Phase 1: Domain Types (Days 1-2)

1. **Create `crates/domain/src/auth.rs`**
   - Define `AuthSpec` enum with all variants
   - Define `OAuth2Token` struct
   - Define `AuthResolution` and `AuthError`
   - Write serialization tests

2. **Create `crates/domain/src/body.rs`**
   - Define `RequestBody` enum
   - Define `FormDataField` enum
   - Define `GraphQLRequest` struct
   - Write serialization tests

3. **Create `crates/domain/src/tls.rs`**
   - Define `TlsConfig` struct
   - Define certificate source types
   - Define security warnings
   - Write tests

### Phase 2: Application Layer (Days 3-4)

4. **Create `crates/application/src/auth/token_store.rs`**
   - Implement in-memory token storage
   - Add expiry tracking
   - Add refresh detection
   - Write comprehensive tests

5. **Create `crates/application/src/auth/provider.rs`**
   - Define `AuthProvider` trait
   - Define `AuthorizationState`
   - Define `AuthEvent` for UI updates

### Phase 3: Infrastructure - Auth (Days 5-6)

6. **Create `crates/infrastructure/src/auth/oauth2_provider.rs`**
   - Implement OAuth2 Client Credentials flow
   - Implement token caching
   - Implement automatic refresh
   - Test with real OAuth2 server

7. **Create `crates/infrastructure/src/auth/callback_server.rs`**
   - Implement local HTTP server
   - Handle OAuth2 callback
   - Generate success/error HTML pages
   - Test authorization code flow

### Phase 4: Infrastructure - Body & TLS (Days 7-8)

8. **Create `crates/infrastructure/src/http/body_builder.rs`**
   - Implement form URL encoding
   - Implement multipart form builder
   - Implement binary body loading
   - Implement GraphQL serialization
   - Test all body types

9. **Create `crates/infrastructure/src/http/tls_config_builder.rs`**
   - Implement CA certificate loading
   - Implement client certificate loading
   - Apply TLS config to reqwest
   - Implement friendly error messages

### Phase 5: UI Components (Days 9-10)

10. **Create `crates/ui/src/components/auth_panel.slint`**
    - Auth type selector
    - Bearer/Basic/ApiKey forms
    - OAuth2 forms with authorize button
    - Token status display

11. **Create `crates/ui/src/components/body_editor.slint`**
    - Body type tabs
    - JSON/Text editor
    - Form editors
    - File picker integration

12. **Create `crates/ui/src/components/tls_settings.slint`**
    - Certificate verification toggles
    - CA certificate list
    - Client certificate configuration
    - Security warnings

### Phase 6: Integration (Days 11-12)

13. **Integrate with request execution flow**
    - Apply auth before sending requests
    - Build body from RequestBody
    - Apply TLS configuration
    - Handle errors gracefully

14. **End-to-end testing**
    - Test OAuth2 flows with mock server
    - Test file uploads
    - Test GraphQL requests
    - Test mTLS connections

---

## Acceptance Criteria

### OAuth2 Implementation
- [ ] Client Credentials flow obtains token automatically
- [ ] Authorization Code flow opens browser and captures callback
- [ ] Tokens are cached in memory per auth configuration
- [ ] Tokens are refreshed automatically before expiry
- [ ] Token status is displayed in UI (valid/expiring/expired)
- [ ] Refresh failures show clear error messages

### Body Types
- [ ] JSON body is serialized with proper Content-Type
- [ ] Form URL-encoded body works with special characters
- [ ] Form-data uploads files correctly
- [ ] Binary body sends file content with correct MIME type
- [ ] GraphQL body includes query, variables, and operation name
- [ ] All body types support variable interpolation (where applicable)

### TLS Configuration
- [ ] Custom CA certificates are trusted
- [ ] Client certificates work for mTLS
- [ ] Insecure mode shows explicit warning
- [ ] Certificate errors show user-friendly messages
- [ ] Settings persist per workspace

### UI Components
- [ ] Auth type selector shows appropriate form for each type
- [ ] OAuth2 authorize button opens browser
- [ ] Token status updates in real-time
- [ ] Body type tabs switch correctly
- [ ] File picker works for form-data and binary
- [ ] TLS settings show security warnings when needed

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| OAuth2 provider incompatibility | High | Test with multiple providers (Auth0, Okta, Keycloak) |
| Callback server port conflicts | Medium | Allow configurable port, try alternatives |
| Large file upload memory issues | Medium | Stream files instead of loading entirely |
| TLS library platform differences | Medium | Test on Windows, macOS, Linux |
| Token refresh race conditions | Medium | Use mutex/lock around refresh operations |

---

## Security Considerations

1. **Token Storage**: Tokens stored only in memory, never persisted to disk in plain text
2. **Secret Logging**: Never log client secrets or access tokens (use `[REDACTED]`)
3. **CSRF Protection**: OAuth2 state parameter validated on callback
4. **TLS Warnings**: Insecure options require explicit user acknowledgment
5. **File Access**: Validate file paths are within allowed directories

---

## Milestone: M5

This sprint completes Milestone 5, enabling production-ready authentication and request bodies for the Vortex API Client.
