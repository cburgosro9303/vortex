# Sprint 02 — Persistencia y Colecciones en Archivos

**Objetivo:** Implementar persistencia de colecciones y requests en disco usando el formato de archivos versionable especificado en `02-file-format-spec.md`.

**Dependencias:** Sprint 00 (arquitectura base), Sprint 01 (modelos Request/Response)

**Milestone:** M2

---

## Alcance

- Formato de archivo `collection.json` v1 con schema versionado
- Guardar y cargar colecciones completas desde disco
- Estructura de directorios navegable (collections/requests/folders)
- IDs estables (UUID v4) para referencias cruzadas
- Serialization deterministica para diffs limpios en Git
- Workspace manifest (`vortex.json`)

## Fuera de Alcance

- Environments y variables (Sprint 03)
- Secrets management (Sprint 03)
- Importacion Postman (Sprint 04)
- Tests/assertions de requests (Sprint 06)

---

## Estructura de Archivos Objetivo

```
my-workspace/
├── vortex.json                    # Workspace manifest
├── collections/
│   └── my-collection/
│       ├── collection.json        # Collection metadata
│       └── requests/
│           ├── get-users.json     # Individual request
│           ├── create-user.json
│           └── auth/              # Folder
│               ├── folder.json    # Folder metadata
│               ├── login.json
│               └── logout.json
├── environments/                  # Reservado (Sprint 03)
└── globals.json                   # Reservado (Sprint 03)
```

---

## Definiciones de Tipos Rust

### Crate: `domain`

Todos los tipos de dominio viven en `domain/src/persistence/`. Sin dependencias externas excepto `serde` para derivacion.

#### Archivo: `domain/src/persistence/mod.rs`

```rust
//! Persistence domain types for Vortex file format v1.
//!
//! These types represent the on-disk format for collections, requests,
//! and workspace configuration. All types use deterministic serialization
//! for clean Git diffs.

mod collection;
mod folder;
mod request;
mod workspace;
mod body;
mod auth;
mod test_assertion;
mod common;

pub use collection::*;
pub use folder::*;
pub use request::*;
pub use workspace::*;
pub use body::*;
pub use auth::*;
pub use test_assertion::*;
pub use common::*;
```

#### Archivo: `domain/src/persistence/common.rs`

```rust
//! Common types shared across persistence models.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Current schema version for all Vortex file formats.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// A UUID string type for stable identifiers.
/// Using String instead of uuid::Uuid to avoid external dependency in domain.
pub type Id = String;

/// Ordered key-value map for deterministic serialization.
/// BTreeMap guarantees alphabetical key ordering.
pub type OrderedMap = BTreeMap<String, String>;

/// Settings that can be applied at various levels (workspace, collection, request).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestSettings {
    /// Request timeout in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    /// Whether to follow HTTP redirects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow_redirects: Option<bool>,

    /// Maximum number of redirects to follow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_redirects: Option<u32>,

    /// Whether to verify SSL certificates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_ssl: Option<bool>,
}

/// HTTP methods supported by Vortex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
    Trace,
}

impl Default for HttpMethod {
    fn default() -> Self {
        Self::Get
    }
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
            Self::Trace => "TRACE",
        };
        write!(f, "{}", s)
    }
}
```

#### Archivo: `domain/src/persistence/workspace.rs`

```rust
//! Workspace manifest type (vortex.json).

use serde::{Deserialize, Serialize};
use super::common::{RequestSettings, CURRENT_SCHEMA_VERSION};

/// Workspace manifest stored in `vortex.json` at project root.
///
/// The workspace defines project-wide settings and lists all collections.
/// Fields are ordered alphabetically for deterministic serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    /// Relative paths to collection directories.
    /// Example: `["collections/users-api", "collections/payments-api"]`
    pub collections: Vec<String>,

    /// Default environment to activate on workspace open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_environment: Option<String>,

    /// Human-readable workspace name.
    pub name: String,

    /// Schema version for migration support.
    pub schema_version: u32,

    /// Global request settings (can be overridden by collection/request).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<RequestSettings>,
}

impl WorkspaceManifest {
    /// Creates a new workspace manifest with default values.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            collections: Vec::new(),
            default_environment: None,
            name: name.into(),
            schema_version: CURRENT_SCHEMA_VERSION,
            settings: Some(RequestSettings {
                timeout_ms: Some(30_000),
                follow_redirects: Some(true),
                max_redirects: Some(10),
                verify_ssl: Some(true),
            }),
        }
    }

    /// Adds a collection path to the workspace.
    pub fn add_collection(&mut self, path: impl Into<String>) {
        self.collections.push(path.into());
    }
}

impl Default for WorkspaceManifest {
    fn default() -> Self {
        Self::new("Untitled Workspace")
    }
}
```

#### Archivo: `domain/src/persistence/collection.rs`

```rust
//! Collection metadata type (collection.json).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use super::auth::Auth;
use super::common::{Id, CURRENT_SCHEMA_VERSION};

/// Collection metadata stored in `collection.json` within a collection directory.
///
/// A collection groups related requests and can define shared authentication
/// and variables that are inherited by all requests within.
///
/// Fields are ordered alphabetically for deterministic serialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    /// Authentication inherited by all requests in this collection.
    /// Can be overridden at folder or request level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,

    /// Human-readable description of the collection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Unique identifier (UUID v4).
    pub id: Id,

    /// Human-readable collection name.
    pub name: String,

    /// Schema version for migration support.
    pub schema_version: u32,

    /// Collection-scoped variables (key-value pairs).
    /// These have lower precedence than environment variables.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, String>,
}

impl Collection {
    /// Creates a new collection with a generated UUID.
    pub fn new(id: Id, name: impl Into<String>) -> Self {
        Self {
            auth: None,
            description: None,
            id,
            name: name.into(),
            schema_version: CURRENT_SCHEMA_VERSION,
            variables: BTreeMap::new(),
        }
    }

    /// Sets the collection description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the collection-level authentication.
    pub fn with_auth(mut self, auth: Auth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Adds a variable to the collection.
    pub fn with_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(key.into(), value.into());
        self
    }
}
```

#### Archivo: `domain/src/persistence/folder.rs`

```rust
//! Folder metadata type (folder.json).

use serde::{Deserialize, Serialize};
use super::auth::Auth;
use super::common::{Id, CURRENT_SCHEMA_VERSION};

/// Folder metadata stored in `folder.json` within a folder directory.
///
/// Folders organize requests hierarchically within a collection.
/// They can define their own auth that overrides collection-level auth.
///
/// Fields are ordered alphabetically for deterministic serialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    /// Authentication inherited by all requests in this folder.
    /// Overrides collection-level auth if set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,

    /// Human-readable description of the folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Unique identifier (UUID v4).
    pub id: Id,

    /// Human-readable folder name.
    pub name: String,

    /// Explicit ordering of requests/subfolders within this folder.
    /// Filenames only (e.g., `["login.json", "logout.json"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,

    /// Schema version for migration support.
    pub schema_version: u32,
}

impl Folder {
    /// Creates a new folder with the given ID and name.
    pub fn new(id: Id, name: impl Into<String>) -> Self {
        Self {
            auth: None,
            description: None,
            id,
            name: name.into(),
            order: Vec::new(),
            schema_version: CURRENT_SCHEMA_VERSION,
        }
    }

    /// Sets the folder description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the folder-level authentication.
    pub fn with_auth(mut self, auth: Auth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Sets the explicit ordering of items in this folder.
    pub fn with_order(mut self, order: Vec<String>) -> Self {
        self.order = order;
        self
    }
}
```

#### Archivo: `domain/src/persistence/request.rs`

```rust
//! Request type for file-based persistence (*.json in requests/).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use super::auth::Auth;
use super::body::RequestBody;
use super::common::{HttpMethod, Id, OrderedMap, RequestSettings, CURRENT_SCHEMA_VERSION};
use super::test_assertion::TestAssertion;

/// A saved HTTP request stored as a JSON file.
///
/// This represents the on-disk format for individual requests.
/// Variables use `{{variable_name}}` syntax and are resolved at execution time.
///
/// Fields are ordered alphabetically for deterministic serialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedRequest {
    /// Request-specific authentication. Overrides folder/collection auth if set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,

    /// Request body (JSON, form data, raw text, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<RequestBody>,

    /// HTTP headers as key-value pairs.
    /// Values may contain `{{variables}}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,

    /// Unique identifier (UUID v4).
    pub id: Id,

    /// HTTP method (GET, POST, PUT, etc.).
    pub method: HttpMethod,

    /// Human-readable request name.
    pub name: String,

    /// URL query parameters as key-value pairs.
    /// Values may contain `{{variables}}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query_params: BTreeMap<String, String>,

    /// Schema version for migration support.
    pub schema_version: u32,

    /// Request-specific settings (timeout, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<RequestSettings>,

    /// Test assertions to run after request execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<TestAssertion>,

    /// Request URL. May contain `{{variables}}`.
    pub url: String,
}

impl SavedRequest {
    /// Creates a new request with required fields.
    pub fn new(id: Id, name: impl Into<String>, method: HttpMethod, url: impl Into<String>) -> Self {
        Self {
            auth: None,
            body: None,
            headers: BTreeMap::new(),
            id,
            method,
            name: name.into(),
            query_params: BTreeMap::new(),
            schema_version: CURRENT_SCHEMA_VERSION,
            settings: None,
            tests: Vec::new(),
            url: url.into(),
        }
    }

    /// Adds a header to the request.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Adds a query parameter to the request.
    pub fn with_query_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query_params.insert(key.into(), value.into());
        self
    }

    /// Sets the request body.
    pub fn with_body(mut self, body: RequestBody) -> Self {
        self.body = Some(body);
        self
    }

    /// Sets the request authentication.
    pub fn with_auth(mut self, auth: Auth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Sets request-specific settings.
    pub fn with_settings(mut self, settings: RequestSettings) -> Self {
        self.settings = Some(settings);
        self
    }

    /// Adds a test assertion.
    pub fn with_test(mut self, test: TestAssertion) -> Self {
        self.tests.push(test);
        self
    }
}

impl Default for SavedRequest {
    fn default() -> Self {
        Self::new(
            String::new(),
            "New Request",
            HttpMethod::Get,
            "https://api.example.com",
        )
    }
}
```

#### Archivo: `domain/src/persistence/body.rs`

```rust
//! Request body types for various content formats.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

/// Request body with multiple format support.
///
/// The `type` field is used as the discriminator for JSON serialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequestBody {
    /// JSON body with structured content.
    Json {
        /// The JSON content (can be object, array, or primitive).
        content: JsonValue,
    },

    /// Plain text body.
    Text {
        /// The text content. May contain `{{variables}}`.
        content: String,
    },

    /// URL-encoded form data (application/x-www-form-urlencoded).
    FormUrlencoded {
        /// Form fields as key-value pairs.
        fields: BTreeMap<String, String>,
    },

    /// Multipart form data (multipart/form-data).
    FormData {
        /// Form fields (text values or file references).
        fields: Vec<FormDataField>,
    },

    /// Binary file body.
    Binary {
        /// Relative path to the binary file.
        path: String,
    },

    /// GraphQL query body.
    Graphql {
        /// The GraphQL query string.
        query: String,
        /// GraphQL variables as JSON object.
        #[serde(skip_serializing_if = "Option::is_none")]
        variables: Option<JsonValue>,
    },
}

impl RequestBody {
    /// Creates a JSON body from a serde_json::Value.
    pub fn json(content: JsonValue) -> Self {
        Self::Json { content }
    }

    /// Creates a text body.
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text { content: content.into() }
    }

    /// Creates a form-urlencoded body from key-value pairs.
    pub fn form_urlencoded(fields: BTreeMap<String, String>) -> Self {
        Self::FormUrlencoded { fields }
    }

    /// Creates a multipart form-data body.
    pub fn form_data(fields: Vec<FormDataField>) -> Self {
        Self::FormData { fields }
    }

    /// Creates a binary body referencing a file path.
    pub fn binary(path: impl Into<String>) -> Self {
        Self::Binary { path: path.into() }
    }

    /// Creates a GraphQL body.
    pub fn graphql(query: impl Into<String>, variables: Option<JsonValue>) -> Self {
        Self::Graphql {
            query: query.into(),
            variables,
        }
    }
}

/// A field in a multipart form-data body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FormDataField {
    /// Text field.
    Text {
        /// Field name.
        name: String,
        /// Field value. May contain `{{variables}}`.
        value: String,
    },
    /// File field.
    File {
        /// Field name.
        name: String,
        /// Relative path to the file.
        path: String,
    },
}

impl FormDataField {
    /// Creates a text field.
    pub fn text(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Text {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Creates a file field.
    pub fn file(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self::File {
            name: name.into(),
            path: path.into(),
        }
    }
}
```

#### Archivo: `domain/src/persistence/auth.rs`

```rust
//! Authentication types for requests and collections.

use serde::{Deserialize, Serialize};

/// Authentication configuration.
///
/// The `type` field is used as the discriminator for JSON serialization.
/// All string values may contain `{{variables}}` for dynamic resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Auth {
    /// Bearer token authentication.
    Bearer {
        /// The bearer token value.
        token: String,
    },

    /// HTTP Basic authentication.
    Basic {
        /// Username for basic auth.
        username: String,
        /// Password for basic auth.
        password: String,
    },

    /// API Key authentication.
    ApiKey {
        /// Header or query parameter name.
        key: String,
        /// The API key value.
        value: String,
        /// Where to send the key: "header" or "query".
        location: ApiKeyLocation,
    },

    /// OAuth2 Client Credentials flow.
    Oauth2ClientCredentials {
        /// Token endpoint URL.
        token_url: String,
        /// Client ID.
        client_id: String,
        /// Client secret.
        client_secret: String,
        /// OAuth scopes (space-separated).
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
    },

    /// OAuth2 Authorization Code flow.
    Oauth2AuthCode {
        /// Authorization endpoint URL.
        auth_url: String,
        /// Token endpoint URL.
        token_url: String,
        /// Client ID.
        client_id: String,
        /// Client secret.
        client_secret: String,
        /// Redirect URI for callback.
        redirect_uri: String,
        /// OAuth scopes (space-separated).
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
    },
}

impl Auth {
    /// Creates a bearer token authentication.
    pub fn bearer(token: impl Into<String>) -> Self {
        Self::Bearer { token: token.into() }
    }

    /// Creates a basic authentication.
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Creates an API key authentication in a header.
    pub fn api_key_header(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::ApiKey {
            key: key.into(),
            value: value.into(),
            location: ApiKeyLocation::Header,
        }
    }

    /// Creates an API key authentication in query params.
    pub fn api_key_query(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::ApiKey {
            key: key.into(),
            value: value.into(),
            location: ApiKeyLocation::Query,
        }
    }
}

/// Location for API key authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyLocation {
    /// Send API key in HTTP header.
    Header,
    /// Send API key in query parameters.
    Query,
}
```

#### Archivo: `domain/src/persistence/test_assertion.rs`

```rust
//! Test assertion types for request validation.
//!
//! Note: Full test execution is Sprint 06 scope.
//! This sprint only defines the types for serialization.

use serde::{Deserialize, Serialize};

/// A test assertion to run after request execution.
///
/// The `type` field is used as the discriminator for JSON serialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TestAssertion {
    /// Assert exact status code.
    Status {
        /// Test name for display.
        name: String,
        /// Expected status code.
        expected: u16,
    },

    /// Assert status code is within range.
    StatusRange {
        /// Test name for display.
        name: String,
        /// Minimum status code (inclusive).
        min: u16,
        /// Maximum status code (inclusive).
        max: u16,
    },

    /// Assert header exists.
    HeaderExists {
        /// Test name for display.
        name: String,
        /// Header name to check.
        header: String,
    },

    /// Assert header has specific value.
    HeaderEquals {
        /// Test name for display.
        name: String,
        /// Header name to check.
        header: String,
        /// Expected header value.
        expected: String,
    },

    /// Assert body contains substring.
    BodyContains {
        /// Test name for display.
        name: String,
        /// Expected substring in body.
        expected: String,
    },

    /// Assert JSON path exists in response.
    JsonPathExists {
        /// Test name for display.
        name: String,
        /// JSONPath expression (e.g., `$.data.id`).
        path: String,
    },

    /// Assert JSON path has specific value.
    JsonPathEquals {
        /// Test name for display.
        name: String,
        /// JSONPath expression.
        path: String,
        /// Expected value at path.
        expected: serde_json::Value,
    },

    /// Assert response time is under threshold.
    ResponseTime {
        /// Test name for display.
        name: String,
        /// Maximum allowed response time in milliseconds.
        max_ms: u64,
    },
}

impl TestAssertion {
    /// Creates a status code assertion.
    pub fn status(name: impl Into<String>, expected: u16) -> Self {
        Self::Status {
            name: name.into(),
            expected,
        }
    }

    /// Creates a status range assertion.
    pub fn status_range(name: impl Into<String>, min: u16, max: u16) -> Self {
        Self::StatusRange {
            name: name.into(),
            min,
            max,
        }
    }

    /// Creates a header exists assertion.
    pub fn header_exists(name: impl Into<String>, header: impl Into<String>) -> Self {
        Self::HeaderExists {
            name: name.into(),
            header: header.into(),
        }
    }

    /// Creates a response time assertion.
    pub fn response_time(name: impl Into<String>, max_ms: u64) -> Self {
        Self::ResponseTime {
            name: name.into(),
            max_ms,
        }
    }
}
```

---

## Serializacion Deterministica

### Crate: `infrastructure`

#### Archivo: `infrastructure/src/serialization/mod.rs`

```rust
//! Deterministic JSON serialization for Vortex file format.
//!
//! Ensures clean Git diffs by:
//! - Sorting object keys alphabetically (via BTreeMap in domain types)
//! - Using 2-space indentation
//! - Adding trailing newline
//! - UTF-8 encoding without BOM

mod json;

pub use json::*;
```

#### Archivo: `infrastructure/src/serialization/json.rs`

```rust
//! JSON serialization helpers for deterministic output.

use serde::{de::DeserializeOwned, Serialize};
use serde_json::ser::{PrettyFormatter, Serializer};
use std::io;

/// Error type for serialization operations.
#[derive(Debug, thiserror::Error)]
pub enum SerializationError {
    #[error("JSON serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("JSON deserialization failed: {0}")]
    Deserialize(serde_json::Error),

    #[error("UTF-8 encoding error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// Serializes a value to deterministic JSON.
///
/// Output format:
/// - 2-space indentation
/// - Trailing newline
/// - Keys sorted alphabetically (requires BTreeMap in source types)
///
/// # Example
///
/// ```rust
/// use infrastructure::serialization::to_json_stable;
/// use domain::persistence::Collection;
///
/// let collection = Collection::new("uuid", "My Collection");
/// let json = to_json_stable(&collection)?;
/// assert!(json.ends_with('\n'));
/// ```
pub fn to_json_stable<T: Serialize>(value: &T) -> Result<String, SerializationError> {
    let mut buffer = Vec::new();
    let formatter = PrettyFormatter::with_indent(b"  ");
    let mut serializer = Serializer::with_formatter(&mut buffer, formatter);
    value.serialize(&mut serializer)?;

    let mut json = String::from_utf8(buffer)?;
    json.push('\n'); // Trailing newline
    Ok(json)
}

/// Serializes a value to deterministic JSON bytes.
///
/// Same as `to_json_stable` but returns bytes for direct file writing.
pub fn to_json_stable_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, SerializationError> {
    let json = to_json_stable(value)?;
    Ok(json.into_bytes())
}

/// Deserializes JSON from a string.
///
/// Handles both pretty-printed and minified JSON.
pub fn from_json<T: DeserializeOwned>(json: &str) -> Result<T, SerializationError> {
    serde_json::from_str(json).map_err(SerializationError::Deserialize)
}

/// Deserializes JSON from bytes.
///
/// Handles both pretty-printed and minified JSON.
pub fn from_json_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SerializationError> {
    serde_json::from_slice(bytes).map_err(SerializationError::Deserialize)
}

/// Validates that JSON can be parsed without deserializing to a specific type.
///
/// Useful for schema validation before attempting typed deserialization.
pub fn validate_json(json: &str) -> Result<serde_json::Value, SerializationError> {
    serde_json::from_str(json).map_err(SerializationError::Deserialize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_stable_serialization_has_trailing_newline() {
        let mut map = BTreeMap::new();
        map.insert("z_key", "value1");
        map.insert("a_key", "value2");

        let json = to_json_stable(&map).unwrap();
        assert!(json.ends_with('\n'));
    }

    #[test]
    fn test_stable_serialization_uses_two_space_indent() {
        let mut map = BTreeMap::new();
        map.insert("key", "value");

        let json = to_json_stable(&map).unwrap();
        assert!(json.contains("  \"key\""));
    }

    #[test]
    fn test_btreemap_keys_are_sorted() {
        let mut map = BTreeMap::new();
        map.insert("zebra", 1);
        map.insert("apple", 2);
        map.insert("mango", 3);

        let json = to_json_stable(&map).unwrap();
        let apple_pos = json.find("apple").unwrap();
        let mango_pos = json.find("mango").unwrap();
        let zebra_pos = json.find("zebra").unwrap();

        assert!(apple_pos < mango_pos);
        assert!(mango_pos < zebra_pos);
    }

    #[test]
    fn test_roundtrip_serialization() {
        let mut original = BTreeMap::new();
        original.insert("key".to_string(), "value".to_string());

        let json = to_json_stable(&original).unwrap();
        let restored: BTreeMap<String, String> = from_json(&json).unwrap();

        assert_eq!(original, restored);
    }
}
```

---

## Repository Traits (Ports)

### Crate: `application`

#### Archivo: `application/src/ports/mod.rs`

```rust
//! Port interfaces (traits) for the application layer.
//!
//! Ports define the contracts between the application and infrastructure.
//! All implementations live in the infrastructure crate.

mod collection_repository;
mod workspace_repository;
mod file_system;

pub use collection_repository::*;
pub use workspace_repository::*;
pub use file_system::*;
```

#### Archivo: `application/src/ports/file_system.rs`

```rust
//! File system abstraction port.

use std::path::{Path, PathBuf};
use async_trait::async_trait;

/// Error type for file system operations.
#[derive(Debug, thiserror::Error)]
pub enum FileSystemError {
    #[error("File not found: {0}")]
    NotFound(PathBuf),

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("Path is not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("Path is not a file: {0}")]
    NotAFile(PathBuf),

    #[error("Path already exists: {0}")]
    AlreadyExists(PathBuf),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Abstraction over file system operations.
///
/// This trait allows mocking file system access in tests.
#[async_trait]
pub trait FileSystem: Send + Sync {
    /// Reads a file's contents as bytes.
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>, FileSystemError>;

    /// Reads a file's contents as a UTF-8 string.
    async fn read_file_string(&self, path: &Path) -> Result<String, FileSystemError>;

    /// Writes bytes to a file, creating it if necessary.
    async fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), FileSystemError>;

    /// Creates a directory and all parent directories.
    async fn create_dir_all(&self, path: &Path) -> Result<(), FileSystemError>;

    /// Checks if a path exists.
    async fn exists(&self, path: &Path) -> bool;

    /// Checks if a path is a directory.
    async fn is_dir(&self, path: &Path) -> bool;

    /// Checks if a path is a file.
    async fn is_file(&self, path: &Path) -> bool;

    /// Lists entries in a directory.
    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FileSystemError>;

    /// Removes a file.
    async fn remove_file(&self, path: &Path) -> Result<(), FileSystemError>;

    /// Removes a directory and all its contents.
    async fn remove_dir_all(&self, path: &Path) -> Result<(), FileSystemError>;

    /// Copies a file from source to destination.
    async fn copy_file(&self, from: &Path, to: &Path) -> Result<(), FileSystemError>;

    /// Renames/moves a file or directory.
    async fn rename(&self, from: &Path, to: &Path) -> Result<(), FileSystemError>;
}
```

#### Archivo: `application/src/ports/workspace_repository.rs`

```rust
//! Workspace repository port.

use std::path::Path;
use async_trait::async_trait;
use domain::persistence::WorkspaceManifest;

/// Error type for workspace operations.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("Workspace not found at: {0}")]
    NotFound(String),

    #[error("Invalid workspace: {0}")]
    Invalid(String),

    #[error("Workspace already exists at: {0}")]
    AlreadyExists(String),

    #[error("Schema version mismatch: expected {expected}, found {found}")]
    SchemaMismatch { expected: u32, found: u32 },

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("File system error: {0}")]
    FileSystem(String),
}

/// Repository for workspace manifest operations.
#[async_trait]
pub trait WorkspaceRepository: Send + Sync {
    /// Loads a workspace manifest from a directory.
    ///
    /// The directory must contain a `vortex.json` file.
    async fn load(&self, workspace_dir: &Path) -> Result<WorkspaceManifest, WorkspaceError>;

    /// Saves a workspace manifest to a directory.
    ///
    /// Creates `vortex.json` in the specified directory.
    async fn save(&self, workspace_dir: &Path, manifest: &WorkspaceManifest) -> Result<(), WorkspaceError>;

    /// Creates a new workspace with initial structure.
    ///
    /// Creates:
    /// - vortex.json
    /// - collections/ directory
    /// - environments/ directory
    /// - .vortex/ directory
    async fn create(&self, workspace_dir: &Path, name: &str) -> Result<WorkspaceManifest, WorkspaceError>;

    /// Checks if a directory contains a valid workspace.
    async fn is_workspace(&self, path: &Path) -> bool;
}
```

#### Archivo: `application/src/ports/collection_repository.rs`

```rust
//! Collection repository port.

use std::path::Path;
use async_trait::async_trait;
use domain::persistence::{Collection, Folder, SavedRequest};

/// Error type for collection operations.
#[derive(Debug, thiserror::Error)]
pub enum CollectionError {
    #[error("Collection not found: {0}")]
    NotFound(String),

    #[error("Request not found: {0}")]
    RequestNotFound(String),

    #[error("Folder not found: {0}")]
    FolderNotFound(String),

    #[error("Invalid collection structure: {0}")]
    InvalidStructure(String),

    #[error("Schema version mismatch: expected {expected}, found {found}")]
    SchemaMismatch { expected: u32, found: u32 },

    #[error("Duplicate ID: {0}")]
    DuplicateId(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("File system error: {0}")]
    FileSystem(String),
}

/// Represents the full tree structure of a loaded collection.
#[derive(Debug, Clone)]
pub struct CollectionTree {
    /// The collection metadata.
    pub collection: Collection,
    /// Root-level requests (in requests/ directory).
    pub requests: Vec<SavedRequest>,
    /// Folders with their nested content.
    pub folders: Vec<FolderTree>,
}

/// A folder with its contents.
#[derive(Debug, Clone)]
pub struct FolderTree {
    /// The folder metadata.
    pub folder: Folder,
    /// Requests in this folder.
    pub requests: Vec<SavedRequest>,
    /// Nested subfolders.
    pub subfolders: Vec<FolderTree>,
    /// Relative path from collection root.
    pub path: String,
}

/// Repository for collection and request file operations.
#[async_trait]
pub trait CollectionRepository: Send + Sync {
    // === Collection Operations ===

    /// Loads a collection and all its contents from disk.
    ///
    /// # Arguments
    /// * `collection_dir` - Path to the collection directory (contains collection.json)
    async fn load_collection(&self, collection_dir: &Path) -> Result<CollectionTree, CollectionError>;

    /// Saves a collection metadata file.
    ///
    /// Only saves the collection.json, not the requests.
    async fn save_collection(&self, collection_dir: &Path, collection: &Collection) -> Result<(), CollectionError>;

    /// Creates a new collection with initial directory structure.
    ///
    /// Creates:
    /// - collection.json
    /// - requests/ directory
    async fn create_collection(&self, collection_dir: &Path, collection: &Collection) -> Result<(), CollectionError>;

    /// Deletes a collection and all its contents.
    async fn delete_collection(&self, collection_dir: &Path) -> Result<(), CollectionError>;

    // === Request Operations ===

    /// Loads a single request from disk.
    ///
    /// # Arguments
    /// * `request_path` - Full path to the request JSON file
    async fn load_request(&self, request_path: &Path) -> Result<SavedRequest, CollectionError>;

    /// Saves a request to disk.
    ///
    /// The filename is derived from the request name (slugified) or can be specified.
    ///
    /// # Arguments
    /// * `request_path` - Full path where the request should be saved
    /// * `request` - The request data to save
    async fn save_request(&self, request_path: &Path, request: &SavedRequest) -> Result<(), CollectionError>;

    /// Creates a new request file in a collection.
    ///
    /// # Arguments
    /// * `collection_dir` - Path to the collection
    /// * `folder_path` - Optional subfolder path (None for root requests/)
    /// * `request` - The request to create
    ///
    /// # Returns
    /// The path where the request was saved
    async fn create_request(
        &self,
        collection_dir: &Path,
        folder_path: Option<&Path>,
        request: &SavedRequest,
    ) -> Result<std::path::PathBuf, CollectionError>;

    /// Deletes a request file.
    async fn delete_request(&self, request_path: &Path) -> Result<(), CollectionError>;

    // === Folder Operations ===

    /// Loads a folder metadata.
    async fn load_folder(&self, folder_path: &Path) -> Result<Folder, CollectionError>;

    /// Saves a folder metadata.
    async fn save_folder(&self, folder_path: &Path, folder: &Folder) -> Result<(), CollectionError>;

    /// Creates a new folder in a collection.
    ///
    /// # Returns
    /// The path to the created folder
    async fn create_folder(
        &self,
        collection_dir: &Path,
        parent_folder: Option<&Path>,
        folder: &Folder,
    ) -> Result<std::path::PathBuf, CollectionError>;

    /// Deletes a folder and all its contents.
    async fn delete_folder(&self, folder_path: &Path) -> Result<(), CollectionError>;
}

/// Helper to generate a filesystem-safe filename from a name.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Get Users"), "get-users");
        assert_eq!(slugify("POST /api/v1/users"), "post-api-v1-users");
        assert_eq!(slugify("  Multiple   Spaces  "), "multiple-spaces");
        assert_eq!(slugify("Special!@#$%Chars"), "special-chars");
    }
}
```

---

## Infrastructure Implementations

### Crate: `infrastructure`

#### Archivo: `infrastructure/src/persistence/mod.rs`

```rust
//! Persistence implementations for file-based storage.

mod file_system;
mod collection_repository;
mod workspace_repository;

pub use file_system::*;
pub use collection_repository::*;
pub use workspace_repository::*;
```

#### Archivo: `infrastructure/src/persistence/file_system.rs`

```rust
//! Real file system implementation.

use application::ports::{FileSystem, FileSystemError};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Real file system implementation using tokio::fs.
#[derive(Debug, Clone, Default)]
pub struct TokioFileSystem;

impl TokioFileSystem {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FileSystem for TokioFileSystem {
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>, FileSystemError> {
        fs::read(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FileSystemError::NotFound(path.to_path_buf())
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                FileSystemError::PermissionDenied(path.to_path_buf())
            } else {
                FileSystemError::Io(e)
            }
        })
    }

    async fn read_file_string(&self, path: &Path) -> Result<String, FileSystemError> {
        fs::read_to_string(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FileSystemError::NotFound(path.to_path_buf())
            } else {
                FileSystemError::Io(e)
            }
        })
    }

    async fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), FileSystemError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, contents).await.map_err(FileSystemError::Io)
    }

    async fn create_dir_all(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::create_dir_all(path).await.map_err(FileSystemError::Io)
    }

    async fn exists(&self, path: &Path) -> bool {
        fs::metadata(path).await.is_ok()
    }

    async fn is_dir(&self, path: &Path) -> bool {
        fs::metadata(path).await.map(|m| m.is_dir()).unwrap_or(false)
    }

    async fn is_file(&self, path: &Path) -> bool {
        fs::metadata(path).await.map(|m| m.is_file()).unwrap_or(false)
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FileSystemError> {
        let mut entries = Vec::new();
        let mut dir = fs::read_dir(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FileSystemError::NotFound(path.to_path_buf())
            } else {
                FileSystemError::Io(e)
            }
        })?;

        while let Some(entry) = dir.next_entry().await? {
            entries.push(entry.path());
        }

        entries.sort(); // Deterministic ordering
        Ok(entries)
    }

    async fn remove_file(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::remove_file(path).await.map_err(FileSystemError::Io)
    }

    async fn remove_dir_all(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::remove_dir_all(path).await.map_err(FileSystemError::Io)
    }

    async fn copy_file(&self, from: &Path, to: &Path) -> Result<(), FileSystemError> {
        fs::copy(from, to).await?;
        Ok(())
    }

    async fn rename(&self, from: &Path, to: &Path) -> Result<(), FileSystemError> {
        fs::rename(from, to).await.map_err(FileSystemError::Io)
    }
}
```

#### Archivo: `infrastructure/src/persistence/collection_repository.rs`

```rust
//! File system based collection repository implementation.

use application::ports::{
    CollectionError, CollectionRepository, CollectionTree, FileSystem, FolderTree, slugify,
};
use async_trait::async_trait;
use domain::persistence::{Collection, Folder, SavedRequest, CURRENT_SCHEMA_VERSION};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::serialization::{from_json, to_json_stable};

/// File names used in the collection structure.
const COLLECTION_FILE: &str = "collection.json";
const FOLDER_FILE: &str = "folder.json";
const REQUESTS_DIR: &str = "requests";

/// File system based implementation of CollectionRepository.
pub struct FileSystemCollectionRepository {
    fs: Arc<dyn FileSystem>,
}

impl FileSystemCollectionRepository {
    /// Creates a new repository with the given file system implementation.
    pub fn new(fs: Arc<dyn FileSystem>) -> Self {
        Self { fs }
    }

    /// Recursively loads a folder and its contents.
    async fn load_folder_tree(&self, folder_path: &Path, relative_path: &str) -> Result<FolderTree, CollectionError> {
        let folder_file = folder_path.join(FOLDER_FILE);
        let folder: Folder = self.load_json(&folder_file).await?;

        let mut requests = Vec::new();
        let mut subfolders = Vec::new();

        let entries = self.fs.read_dir(folder_path).await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

        for entry in entries {
            let file_name = entry.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_default();

            if file_name == FOLDER_FILE {
                continue;
            }

            if self.fs.is_dir(&entry).await {
                // Check if it's a subfolder (has folder.json)
                let subfolder_meta = entry.join(FOLDER_FILE);
                if self.fs.exists(&subfolder_meta).await {
                    let sub_relative = format!("{}/{}", relative_path, file_name);
                    let subfolder_tree = Box::pin(self.load_folder_tree(&entry, &sub_relative)).await?;
                    subfolders.push(subfolder_tree);
                }
            } else if file_name.ends_with(".json") {
                // It's a request file
                let request = self.load_request(&entry).await?;
                requests.push(request);
            }
        }

        Ok(FolderTree {
            folder,
            requests,
            subfolders,
            path: relative_path.to_string(),
        })
    }

    /// Loads and deserializes a JSON file.
    async fn load_json<T: serde::de::DeserializeOwned>(&self, path: &Path) -> Result<T, CollectionError> {
        let content = self.fs.read_file_string(path).await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;
        from_json(&content).map_err(|e| CollectionError::Serialization(e.to_string()))
    }

    /// Serializes and saves a value to a JSON file.
    async fn save_json<T: serde::Serialize>(&self, path: &Path, value: &T) -> Result<(), CollectionError> {
        let json = to_json_stable(value)
            .map_err(|e| CollectionError::Serialization(e.to_string()))?;
        self.fs.write_file(path, json.as_bytes()).await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))
    }
}

#[async_trait]
impl CollectionRepository for FileSystemCollectionRepository {
    async fn load_collection(&self, collection_dir: &Path) -> Result<CollectionTree, CollectionError> {
        let collection_file = collection_dir.join(COLLECTION_FILE);

        if !self.fs.exists(&collection_file).await {
            return Err(CollectionError::NotFound(collection_dir.display().to_string()));
        }

        let collection: Collection = self.load_json(&collection_file).await?;

        // Validate schema version
        if collection.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(CollectionError::SchemaMismatch {
                expected: CURRENT_SCHEMA_VERSION,
                found: collection.schema_version,
            });
        }

        let requests_dir = collection_dir.join(REQUESTS_DIR);
        let mut requests = Vec::new();
        let mut folders = Vec::new();

        if self.fs.exists(&requests_dir).await {
            let entries = self.fs.read_dir(&requests_dir).await
                .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

            for entry in entries {
                let file_name = entry.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                if self.fs.is_dir(&entry).await {
                    // Check if it's a folder (has folder.json)
                    let folder_meta = entry.join(FOLDER_FILE);
                    if self.fs.exists(&folder_meta).await {
                        let folder_tree = self.load_folder_tree(&entry, &file_name).await?;
                        folders.push(folder_tree);
                    }
                } else if file_name.ends_with(".json") {
                    let request = self.load_request(&entry).await?;
                    requests.push(request);
                }
            }
        }

        Ok(CollectionTree {
            collection,
            requests,
            folders,
        })
    }

    async fn save_collection(&self, collection_dir: &Path, collection: &Collection) -> Result<(), CollectionError> {
        let collection_file = collection_dir.join(COLLECTION_FILE);
        self.save_json(&collection_file, collection).await
    }

    async fn create_collection(&self, collection_dir: &Path, collection: &Collection) -> Result<(), CollectionError> {
        if self.fs.exists(collection_dir).await {
            return Err(CollectionError::InvalidStructure(
                format!("Directory already exists: {}", collection_dir.display())
            ));
        }

        // Create directory structure
        self.fs.create_dir_all(collection_dir).await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

        let requests_dir = collection_dir.join(REQUESTS_DIR);
        self.fs.create_dir_all(&requests_dir).await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

        // Save collection metadata
        self.save_collection(collection_dir, collection).await
    }

    async fn delete_collection(&self, collection_dir: &Path) -> Result<(), CollectionError> {
        self.fs.remove_dir_all(collection_dir).await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))
    }

    async fn load_request(&self, request_path: &Path) -> Result<SavedRequest, CollectionError> {
        if !self.fs.exists(request_path).await {
            return Err(CollectionError::RequestNotFound(request_path.display().to_string()));
        }
        self.load_json(request_path).await
    }

    async fn save_request(&self, request_path: &Path, request: &SavedRequest) -> Result<(), CollectionError> {
        self.save_json(request_path, request).await
    }

    async fn create_request(
        &self,
        collection_dir: &Path,
        folder_path: Option<&Path>,
        request: &SavedRequest,
    ) -> Result<PathBuf, CollectionError> {
        let base_dir = match folder_path {
            Some(folder) => collection_dir.join(REQUESTS_DIR).join(folder),
            None => collection_dir.join(REQUESTS_DIR),
        };

        // Ensure directory exists
        self.fs.create_dir_all(&base_dir).await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

        // Generate filename from request name
        let filename = format!("{}.json", slugify(&request.name));
        let request_path = base_dir.join(&filename);

        // Check for duplicates
        if self.fs.exists(&request_path).await {
            return Err(CollectionError::DuplicateId(request_path.display().to_string()));
        }

        self.save_request(&request_path, request).await?;
        Ok(request_path)
    }

    async fn delete_request(&self, request_path: &Path) -> Result<(), CollectionError> {
        self.fs.remove_file(request_path).await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))
    }

    async fn load_folder(&self, folder_path: &Path) -> Result<Folder, CollectionError> {
        let folder_file = folder_path.join(FOLDER_FILE);
        if !self.fs.exists(&folder_file).await {
            return Err(CollectionError::FolderNotFound(folder_path.display().to_string()));
        }
        self.load_json(&folder_file).await
    }

    async fn save_folder(&self, folder_path: &Path, folder: &Folder) -> Result<(), CollectionError> {
        let folder_file = folder_path.join(FOLDER_FILE);
        self.save_json(&folder_file, folder).await
    }

    async fn create_folder(
        &self,
        collection_dir: &Path,
        parent_folder: Option<&Path>,
        folder: &Folder,
    ) -> Result<PathBuf, CollectionError> {
        let base_dir = match parent_folder {
            Some(parent) => collection_dir.join(REQUESTS_DIR).join(parent),
            None => collection_dir.join(REQUESTS_DIR),
        };

        let folder_name = slugify(&folder.name);
        let folder_path = base_dir.join(&folder_name);

        if self.fs.exists(&folder_path).await {
            return Err(CollectionError::InvalidStructure(
                format!("Folder already exists: {}", folder_path.display())
            ));
        }

        self.fs.create_dir_all(&folder_path).await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

        self.save_folder(&folder_path, folder).await?;
        Ok(folder_path)
    }

    async fn delete_folder(&self, folder_path: &Path) -> Result<(), CollectionError> {
        self.fs.remove_dir_all(folder_path).await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))
    }
}
```

#### Archivo: `infrastructure/src/persistence/workspace_repository.rs`

```rust
//! File system based workspace repository implementation.

use application::ports::{FileSystem, WorkspaceError, WorkspaceRepository};
use async_trait::async_trait;
use domain::persistence::{WorkspaceManifest, CURRENT_SCHEMA_VERSION};
use std::path::Path;
use std::sync::Arc;

use crate::serialization::{from_json, to_json_stable};

const WORKSPACE_FILE: &str = "vortex.json";
const COLLECTIONS_DIR: &str = "collections";
const ENVIRONMENTS_DIR: &str = "environments";
const VORTEX_DIR: &str = ".vortex";

/// File system based implementation of WorkspaceRepository.
pub struct FileSystemWorkspaceRepository {
    fs: Arc<dyn FileSystem>,
}

impl FileSystemWorkspaceRepository {
    /// Creates a new repository with the given file system implementation.
    pub fn new(fs: Arc<dyn FileSystem>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl WorkspaceRepository for FileSystemWorkspaceRepository {
    async fn load(&self, workspace_dir: &Path) -> Result<WorkspaceManifest, WorkspaceError> {
        let manifest_path = workspace_dir.join(WORKSPACE_FILE);

        if !self.fs.exists(&manifest_path).await {
            return Err(WorkspaceError::NotFound(workspace_dir.display().to_string()));
        }

        let content = self.fs.read_file_string(&manifest_path).await
            .map_err(|e| WorkspaceError::FileSystem(e.to_string()))?;

        let manifest: WorkspaceManifest = from_json(&content)
            .map_err(|e| WorkspaceError::Serialization(e.to_string()))?;

        // Validate schema version
        if manifest.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(WorkspaceError::SchemaMismatch {
                expected: CURRENT_SCHEMA_VERSION,
                found: manifest.schema_version,
            });
        }

        Ok(manifest)
    }

    async fn save(&self, workspace_dir: &Path, manifest: &WorkspaceManifest) -> Result<(), WorkspaceError> {
        let manifest_path = workspace_dir.join(WORKSPACE_FILE);

        let json = to_json_stable(manifest)
            .map_err(|e| WorkspaceError::Serialization(e.to_string()))?;

        self.fs.write_file(&manifest_path, json.as_bytes()).await
            .map_err(|e| WorkspaceError::FileSystem(e.to_string()))
    }

    async fn create(&self, workspace_dir: &Path, name: &str) -> Result<WorkspaceManifest, WorkspaceError> {
        if self.fs.exists(&workspace_dir.join(WORKSPACE_FILE)).await {
            return Err(WorkspaceError::AlreadyExists(workspace_dir.display().to_string()));
        }

        // Create directory structure
        self.fs.create_dir_all(workspace_dir).await
            .map_err(|e| WorkspaceError::FileSystem(e.to_string()))?;

        self.fs.create_dir_all(&workspace_dir.join(COLLECTIONS_DIR)).await
            .map_err(|e| WorkspaceError::FileSystem(e.to_string()))?;

        self.fs.create_dir_all(&workspace_dir.join(ENVIRONMENTS_DIR)).await
            .map_err(|e| WorkspaceError::FileSystem(e.to_string()))?;

        self.fs.create_dir_all(&workspace_dir.join(VORTEX_DIR)).await
            .map_err(|e| WorkspaceError::FileSystem(e.to_string()))?;

        // Create manifest
        let manifest = WorkspaceManifest::new(name);
        self.save(workspace_dir, &manifest).await?;

        Ok(manifest)
    }

    async fn is_workspace(&self, path: &Path) -> bool {
        self.fs.exists(&path.join(WORKSPACE_FILE)).await
    }
}
```

---

## Use Cases

### Crate: `application`

#### Archivo: `application/src/use_cases/mod.rs`

```rust
//! Application use cases (business logic orchestration).

mod save_collection;
mod load_collection;
mod create_request;
mod update_request;
mod create_workspace;

pub use save_collection::*;
pub use load_collection::*;
pub use create_request::*;
pub use update_request::*;
pub use create_workspace::*;
```

#### Archivo: `application/src/use_cases/create_workspace.rs`

```rust
//! Create workspace use case.

use crate::ports::{WorkspaceError, WorkspaceRepository};
use domain::persistence::WorkspaceManifest;
use std::path::Path;
use std::sync::Arc;

/// Input for creating a new workspace.
#[derive(Debug, Clone)]
pub struct CreateWorkspaceInput {
    /// Directory where the workspace will be created.
    pub path: std::path::PathBuf,
    /// Name of the workspace.
    pub name: String,
}

/// Use case for creating a new workspace.
pub struct CreateWorkspace {
    workspace_repo: Arc<dyn WorkspaceRepository>,
}

impl CreateWorkspace {
    pub fn new(workspace_repo: Arc<dyn WorkspaceRepository>) -> Self {
        Self { workspace_repo }
    }

    /// Creates a new workspace at the specified path.
    ///
    /// # Errors
    /// - Returns error if workspace already exists at path
    /// - Returns error if directory creation fails
    pub async fn execute(&self, input: CreateWorkspaceInput) -> Result<WorkspaceManifest, WorkspaceError> {
        self.workspace_repo.create(&input.path, &input.name).await
    }
}
```

#### Archivo: `application/src/use_cases/save_collection.rs`

```rust
//! Save collection use case.

use crate::ports::{CollectionError, CollectionRepository};
use domain::persistence::Collection;
use std::path::Path;
use std::sync::Arc;

/// Input for saving a collection.
#[derive(Debug, Clone)]
pub struct SaveCollectionInput {
    /// Path to the collection directory.
    pub collection_dir: std::path::PathBuf,
    /// The collection metadata to save.
    pub collection: Collection,
    /// Whether to create the collection if it doesn't exist.
    pub create_if_missing: bool,
}

/// Use case for saving a collection to disk.
pub struct SaveCollection {
    collection_repo: Arc<dyn CollectionRepository>,
}

impl SaveCollection {
    pub fn new(collection_repo: Arc<dyn CollectionRepository>) -> Self {
        Self { collection_repo }
    }

    /// Saves the collection metadata to disk.
    ///
    /// If `create_if_missing` is true and the collection doesn't exist,
    /// creates the full directory structure.
    ///
    /// # Errors
    /// - Returns error if collection doesn't exist and create_if_missing is false
    /// - Returns error if file system operations fail
    pub async fn execute(&self, input: SaveCollectionInput) -> Result<(), CollectionError> {
        let collection_file = input.collection_dir.join("collection.json");

        // Check if this is a new collection
        let exists = tokio::fs::metadata(&collection_file).await.is_ok();

        if !exists && input.create_if_missing {
            self.collection_repo.create_collection(&input.collection_dir, &input.collection).await
        } else if !exists {
            Err(CollectionError::NotFound(input.collection_dir.display().to_string()))
        } else {
            self.collection_repo.save_collection(&input.collection_dir, &input.collection).await
        }
    }
}
```

#### Archivo: `application/src/use_cases/load_collection.rs`

```rust
//! Load collection use case.

use crate::ports::{CollectionError, CollectionRepository, CollectionTree};
use std::path::Path;
use std::sync::Arc;

/// Input for loading a collection.
#[derive(Debug, Clone)]
pub struct LoadCollectionInput {
    /// Path to the collection directory.
    pub collection_dir: std::path::PathBuf,
}

/// Use case for loading a collection from disk.
pub struct LoadCollection {
    collection_repo: Arc<dyn CollectionRepository>,
}

impl LoadCollection {
    pub fn new(collection_repo: Arc<dyn CollectionRepository>) -> Self {
        Self { collection_repo }
    }

    /// Loads a collection and all its contents from disk.
    ///
    /// # Returns
    /// A `CollectionTree` containing the collection metadata,
    /// all requests, and nested folder structure.
    ///
    /// # Errors
    /// - Returns error if collection doesn't exist
    /// - Returns error if schema version is unsupported
    /// - Returns error if JSON parsing fails
    pub async fn execute(&self, input: LoadCollectionInput) -> Result<CollectionTree, CollectionError> {
        self.collection_repo.load_collection(&input.collection_dir).await
    }
}
```

#### Archivo: `application/src/use_cases/create_request.rs`

```rust
//! Create request use case.

use crate::ports::{CollectionError, CollectionRepository};
use domain::persistence::SavedRequest;
use std::path::PathBuf;
use std::sync::Arc;

/// Input for creating a new request.
#[derive(Debug, Clone)]
pub struct CreateRequestInput {
    /// Path to the collection directory.
    pub collection_dir: PathBuf,
    /// Optional folder path within the collection (relative to requests/).
    pub folder_path: Option<PathBuf>,
    /// The request to create.
    pub request: SavedRequest,
}

/// Output from creating a request.
#[derive(Debug, Clone)]
pub struct CreateRequestOutput {
    /// The path where the request was saved.
    pub request_path: PathBuf,
    /// The saved request (with any modifications).
    pub request: SavedRequest,
}

/// Use case for creating a new request in a collection.
pub struct CreateRequest {
    collection_repo: Arc<dyn CollectionRepository>,
}

impl CreateRequest {
    pub fn new(collection_repo: Arc<dyn CollectionRepository>) -> Self {
        Self { collection_repo }
    }

    /// Creates a new request file in the collection.
    ///
    /// The request will be saved as `{slugified-name}.json` in the
    /// appropriate directory (requests/ or requests/{folder}/).
    ///
    /// # Errors
    /// - Returns error if a request with the same name already exists
    /// - Returns error if the collection or folder doesn't exist
    /// - Returns error if file system operations fail
    pub async fn execute(&self, input: CreateRequestInput) -> Result<CreateRequestOutput, CollectionError> {
        let request_path = self.collection_repo.create_request(
            &input.collection_dir,
            input.folder_path.as_deref(),
            &input.request,
        ).await?;

        Ok(CreateRequestOutput {
            request_path,
            request: input.request,
        })
    }
}
```

#### Archivo: `application/src/use_cases/update_request.rs`

```rust
//! Update request use case.

use crate::ports::{CollectionError, CollectionRepository, slugify};
use domain::persistence::SavedRequest;
use std::path::PathBuf;
use std::sync::Arc;

/// Input for updating a request.
#[derive(Debug, Clone)]
pub struct UpdateRequestInput {
    /// Current path to the request file.
    pub request_path: PathBuf,
    /// The updated request data.
    pub request: SavedRequest,
    /// Whether to rename the file if the request name changed.
    pub rename_file: bool,
}

/// Output from updating a request.
#[derive(Debug, Clone)]
pub struct UpdateRequestOutput {
    /// The path where the request is now saved (may differ if renamed).
    pub request_path: PathBuf,
}

/// Use case for updating an existing request.
pub struct UpdateRequest {
    collection_repo: Arc<dyn CollectionRepository>,
}

impl UpdateRequest {
    pub fn new(collection_repo: Arc<dyn CollectionRepository>) -> Self {
        Self { collection_repo }
    }

    /// Updates a request file on disk.
    ///
    /// If `rename_file` is true and the request name has changed,
    /// the file will be renamed to match the new name.
    ///
    /// # Errors
    /// - Returns error if the request file doesn't exist
    /// - Returns error if file rename conflicts with existing file
    /// - Returns error if file system operations fail
    pub async fn execute(&self, input: UpdateRequestInput) -> Result<UpdateRequestOutput, CollectionError> {
        // Determine if we need to rename
        let current_stem = input.request_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let new_stem = slugify(&input.request);

        let final_path = if input.rename_file && current_stem != new_stem {
            // Need to rename the file
            let parent = input.request_path.parent()
                .ok_or_else(|| CollectionError::InvalidStructure("Invalid request path".into()))?;
            let new_path = parent.join(format!("{}.json", new_stem));

            // Delete old file, save to new location
            self.collection_repo.delete_request(&input.request_path).await?;
            self.collection_repo.save_request(&new_path, &input.request).await?;
            new_path
        } else {
            // Save in place
            self.collection_repo.save_request(&input.request_path, &input.request).await?;
            input.request_path
        };

        Ok(UpdateRequestOutput {
            request_path: final_path,
        })
    }
}
```

---

## ID Generation

#### Archivo: `domain/src/id.rs`

```rust
//! ID generation utilities.

use uuid::Uuid;

/// Generates a new UUID v4 as a string.
///
/// This is the standard ID format for all Vortex entities.
pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id_format() {
        let id = generate_id();
        // UUID v4 format: 8-4-4-4-12 = 36 chars
        assert_eq!(id.len(), 36);
        assert!(Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn test_generate_id_uniqueness() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
    }
}
```

---

## UI Components (Slint)

### Archivo: `ui/src/components/file_dialog.slint`

```slint
// File dialog component for selecting directories and files.
// Uses native file picker via Rust callback.

import { Button, VerticalBox, HorizontalBox, LineEdit } from "std-widgets.slint";

export component FilePathInput inherits Rectangle {
    in-out property <string> path;
    in property <string> placeholder: "Select a path...";
    in property <string> button-text: "Browse";
    in property <bool> enabled: true;

    callback browse-clicked();
    callback path-changed(string);

    HorizontalBox {
        spacing: 8px;

        LineEdit {
            text: path;
            placeholder-text: placeholder;
            enabled: root.enabled;
            edited(new-text) => {
                root.path = new-text;
                root.path-changed(new-text);
            }
        }

        Button {
            text: button-text;
            enabled: root.enabled;
            clicked => {
                root.browse-clicked();
            }
        }
    }
}

export component WorkspaceSelector inherits Rectangle {
    in-out property <string> workspace-path;
    in property <bool> has-workspace: workspace-path != "";

    callback open-workspace();
    callback create-workspace();
    callback close-workspace();

    VerticalBox {
        spacing: 12px;
        padding: 16px;

        Text {
            text: has-workspace ? "Current Workspace" : "No Workspace Open";
            font-size: 14px;
            font-weight: 600;
        }

        if has-workspace: Text {
            text: workspace-path;
            font-size: 12px;
            color: #666;
            overflow: elide;
        }

        HorizontalBox {
            spacing: 8px;

            Button {
                text: "Open Workspace";
                clicked => { root.open-workspace(); }
            }

            Button {
                text: "New Workspace";
                clicked => { root.create-workspace(); }
            }

            if has-workspace: Button {
                text: "Close";
                clicked => { root.close-workspace(); }
            }
        }
    }
}
```

### Archivo: `ui/src/components/collection_tree.slint`

```slint
// Collection tree view for navigating requests and folders.

import { Button, VerticalBox, HorizontalBox, ScrollView } from "std-widgets.slint";

// Represents an item in the collection tree (request or folder)
export struct TreeItem {
    id: string,
    name: string,
    item-type: string,  // "request", "folder", "collection"
    method: string,     // HTTP method for requests, empty for folders
    depth: int,         // Nesting level for indentation
    expanded: bool,     // For folders: whether children are visible
    path: string,       // File path
}

export component TreeItemRow inherits Rectangle {
    in property <TreeItem> item;
    in property <bool> selected: false;

    callback clicked();
    callback double-clicked();
    callback toggle-expanded();
    callback context-menu(/* x */ length, /* y */ length);

    min-height: 32px;
    background: selected ? #e3f2fd : transparent;

    TouchArea {
        clicked => { root.clicked(); }
        double-clicked => { root.double-clicked(); }
        pointer-event(event) => {
            if event.button == PointerEventButton.right {
                root.context-menu(event.x, event.y);
            }
        }
    }

    HorizontalBox {
        padding-left: (item.depth * 16px) + 8px;
        padding-right: 8px;
        spacing: 8px;
        alignment: start;

        // Expand/collapse button for folders
        if item.item-type == "folder" || item.item-type == "collection": Rectangle {
            width: 20px;
            height: 20px;

            Text {
                text: item.expanded ? "v" : ">";
                font-size: 12px;
                horizontal-alignment: center;
                vertical-alignment: center;
            }

            TouchArea {
                clicked => { root.toggle-expanded(); }
            }
        }

        // Method badge for requests
        if item.item-type == "request": Rectangle {
            width: 48px;
            height: 20px;
            border-radius: 4px;
            background: item.method == "GET" ? #4caf50 :
                       item.method == "POST" ? #2196f3 :
                       item.method == "PUT" ? #ff9800 :
                       item.method == "PATCH" ? #9c27b0 :
                       item.method == "DELETE" ? #f44336 :
                       #757575;

            Text {
                text: item.method;
                font-size: 10px;
                font-weight: 600;
                color: white;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        // Folder icon
        if item.item-type == "folder": Text {
            text: "[]";
            font-size: 14px;
            color: #ffc107;
        }

        // Collection icon
        if item.item-type == "collection": Text {
            text: "{}";
            font-size: 14px;
            color: #2196f3;
        }

        // Item name
        Text {
            text: item.name;
            font-size: 13px;
            overflow: elide;
            vertical-alignment: center;
        }
    }
}

export component CollectionTreeView inherits Rectangle {
    in property <[TreeItem]> items;
    in-out property <string> selected-id;

    callback item-selected(TreeItem);
    callback item-double-clicked(TreeItem);
    callback toggle-folder(TreeItem);
    callback create-request(TreeItem /* parent */);
    callback create-folder(TreeItem /* parent */);
    callback delete-item(TreeItem);
    callback rename-item(TreeItem);

    ScrollView {
        VerticalBox {
            alignment: start;
            spacing: 2px;

            for item in items: TreeItemRow {
                item: item;
                selected: item.id == selected-id;
                clicked => {
                    selected-id = item.id;
                    root.item-selected(item);
                }
                double-clicked => {
                    root.item-double-clicked(item);
                }
                toggle-expanded => {
                    root.toggle-folder(item);
                }
            }
        }
    }
}
```

### Archivo: `ui/src/components/save_open_buttons.slint`

```slint
// Save and open buttons for file operations.

import { Button, HorizontalBox } from "std-widgets.slint";

export component FileOperationButtons inherits Rectangle {
    in property <bool> has-changes: false;
    in property <bool> saving: false;
    in property <bool> enabled: true;

    callback save-clicked();
    callback save-as-clicked();
    callback open-clicked();

    HorizontalBox {
        spacing: 8px;
        padding: 8px;

        Button {
            text: saving ? "Saving..." : (has-changes ? "Save *" : "Save");
            enabled: root.enabled && !saving;
            clicked => { root.save-clicked(); }
        }

        Button {
            text: "Save As...";
            enabled: root.enabled && !saving;
            clicked => { root.save-as-clicked(); }
        }

        Button {
            text: "Open...";
            enabled: root.enabled && !saving;
            clicked => { root.open-clicked(); }
        }
    }
}

export component CollectionToolbar inherits Rectangle {
    in property <string> collection-name;
    in property <bool> has-changes: false;
    in property <bool> saving: false;

    callback save();
    callback new-request();
    callback new-folder();
    callback settings();

    background: #f5f5f5;
    min-height: 48px;

    HorizontalBox {
        padding: 8px;
        spacing: 12px;
        alignment: space-between;

        HorizontalBox {
            spacing: 8px;
            alignment: start;

            Text {
                text: collection-name;
                font-size: 16px;
                font-weight: 600;
                vertical-alignment: center;
            }

            if has-changes: Text {
                text: "(unsaved)";
                font-size: 12px;
                color: #ff9800;
                vertical-alignment: center;
            }
        }

        HorizontalBox {
            spacing: 8px;
            alignment: end;

            Button {
                text: "+ Request";
                clicked => { root.new-request(); }
            }

            Button {
                text: "+ Folder";
                clicked => { root.new-folder(); }
            }

            Button {
                text: saving ? "Saving..." : "Save";
                enabled: has-changes && !saving;
                clicked => { root.save(); }
            }

            Button {
                text: "...";
                clicked => { root.settings(); }
            }
        }
    }
}
```

---

## UI State Management

### Archivo: `ui/src/state/collection_state.rs`

```rust
//! UI state for collection management.

use domain::persistence::{Collection, Folder, SavedRequest};
use std::collections::HashMap;
use std::path::PathBuf;

/// Represents a node in the UI tree view.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: String,
    pub name: String,
    pub node_type: TreeNodeType,
    pub depth: u32,
    pub expanded: bool,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreeNodeType {
    Collection,
    Folder,
    Request { method: String },
}

/// State for the collection sidebar.
#[derive(Debug, Default)]
pub struct CollectionState {
    /// Currently loaded workspace path.
    pub workspace_path: Option<PathBuf>,
    /// Loaded collections indexed by path.
    pub collections: HashMap<PathBuf, CollectionData>,
    /// Currently selected item ID.
    pub selected_id: Option<String>,
    /// Expanded folder IDs.
    pub expanded_ids: std::collections::HashSet<String>,
    /// Items with unsaved changes.
    pub dirty_ids: std::collections::HashSet<String>,
}

/// Data for a loaded collection.
#[derive(Debug, Clone)]
pub struct CollectionData {
    pub collection: Collection,
    pub requests: Vec<SavedRequest>,
    pub folders: Vec<FolderData>,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct FolderData {
    pub folder: Folder,
    pub requests: Vec<SavedRequest>,
    pub subfolders: Vec<FolderData>,
    pub path: PathBuf,
}

impl CollectionState {
    /// Creates a new empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Flattens the collection tree into a list for UI rendering.
    pub fn flatten_tree(&self) -> Vec<TreeNode> {
        let mut nodes = Vec::new();

        for (path, data) in &self.collections {
            let collection_id = data.collection.id.clone();
            let expanded = self.expanded_ids.contains(&collection_id);

            nodes.push(TreeNode {
                id: collection_id.clone(),
                name: data.collection.name.clone(),
                node_type: TreeNodeType::Collection,
                depth: 0,
                expanded,
                path: path.clone(),
            });

            if expanded {
                // Add root-level requests
                for request in &data.requests {
                    nodes.push(TreeNode {
                        id: request.id.clone(),
                        name: request.name.clone(),
                        node_type: TreeNodeType::Request {
                            method: request.method.to_string(),
                        },
                        depth: 1,
                        expanded: false,
                        path: path.join("requests").join(format!("{}.json",
                            application::ports::slugify(&request.name))),
                    });
                }

                // Add folders recursively
                self.flatten_folders(&data.folders, path, 1, &mut nodes);
            }
        }

        nodes
    }

    fn flatten_folders(
        &self,
        folders: &[FolderData],
        base_path: &PathBuf,
        depth: u32,
        nodes: &mut Vec<TreeNode>,
    ) {
        for folder_data in folders {
            let folder_id = folder_data.folder.id.clone();
            let expanded = self.expanded_ids.contains(&folder_id);

            nodes.push(TreeNode {
                id: folder_id.clone(),
                name: folder_data.folder.name.clone(),
                node_type: TreeNodeType::Folder,
                depth,
                expanded,
                path: folder_data.path.clone(),
            });

            if expanded {
                // Add folder's requests
                for request in &folder_data.requests {
                    nodes.push(TreeNode {
                        id: request.id.clone(),
                        name: request.name.clone(),
                        node_type: TreeNodeType::Request {
                            method: request.method.to_string(),
                        },
                        depth: depth + 1,
                        expanded: false,
                        path: folder_data.path.join(format!("{}.json",
                            application::ports::slugify(&request.name))),
                    });
                }

                // Recurse into subfolders
                self.flatten_folders(&folder_data.subfolders, &folder_data.path, depth + 1, nodes);
            }
        }
    }

    /// Toggles the expanded state of a folder or collection.
    pub fn toggle_expanded(&mut self, id: &str) {
        if self.expanded_ids.contains(id) {
            self.expanded_ids.remove(id);
        } else {
            self.expanded_ids.insert(id.to_string());
        }
    }

    /// Marks an item as having unsaved changes.
    pub fn mark_dirty(&mut self, id: &str) {
        self.dirty_ids.insert(id.to_string());
    }

    /// Clears the dirty flag for an item.
    pub fn mark_clean(&mut self, id: &str) {
        self.dirty_ids.remove(id);
    }

    /// Returns whether any items have unsaved changes.
    pub fn has_unsaved_changes(&self) -> bool {
        !self.dirty_ids.is_empty()
    }
}
```

---

## Checklist de Tareas

### Fase 1: Tipos de Dominio (Dias 1-2)

- [ ] **1.1** Crear modulo `domain/src/persistence/mod.rs` con re-exports
- [ ] **1.2** Implementar `common.rs` con tipos compartidos (`HttpMethod`, `RequestSettings`, `OrderedMap`)
- [ ] **1.3** Implementar `workspace.rs` con `WorkspaceManifest`
- [ ] **1.4** Implementar `collection.rs` con `Collection`
- [ ] **1.5** Implementar `folder.rs` con `Folder`
- [ ] **1.6** Implementar `request.rs` con `SavedRequest`
- [ ] **1.7** Implementar `body.rs` con `RequestBody` y variantes
- [ ] **1.8** Implementar `auth.rs` con `Auth` y variantes
- [ ] **1.9** Implementar `test_assertion.rs` con `TestAssertion` (tipos solo, ejecucion en Sprint 06)
- [ ] **1.10** Implementar `domain/src/id.rs` para generacion de UUIDs
- [ ] **1.11** Agregar `uuid` y `serde_json` a dependencias de domain

### Fase 2: Serializacion (Dias 2-3)

- [ ] **2.1** Crear modulo `infrastructure/src/serialization/mod.rs`
- [ ] **2.2** Implementar `json.rs` con `to_json_stable` y `from_json`
- [ ] **2.3** Agregar `thiserror` a dependencias de infrastructure
- [ ] **2.4** Escribir tests unitarios para serializacion deterministica
- [ ] **2.5** Verificar ordenamiento alfabetico de campos con BTreeMap
- [ ] **2.6** Verificar indentacion de 2 espacios y newline final

### Fase 3: Ports (Interfaces) (Dias 3-4)

- [ ] **3.1** Crear modulo `application/src/ports/mod.rs`
- [ ] **3.2** Implementar `file_system.rs` con trait `FileSystem`
- [ ] **3.3** Implementar `workspace_repository.rs` con trait `WorkspaceRepository`
- [ ] **3.4** Implementar `collection_repository.rs` con trait `CollectionRepository`
- [ ] **3.5** Definir tipos `CollectionTree` y `FolderTree`
- [ ] **3.6** Implementar funcion `slugify` para nombres de archivo
- [ ] **3.7** Agregar `async-trait` a dependencias de application

### Fase 4: Implementaciones de Infrastructure (Dias 4-6)

- [ ] **4.1** Crear modulo `infrastructure/src/persistence/mod.rs`
- [ ] **4.2** Implementar `TokioFileSystem` en `file_system.rs`
- [ ] **4.3** Implementar `FileSystemWorkspaceRepository` en `workspace_repository.rs`
- [ ] **4.4** Implementar `FileSystemCollectionRepository` en `collection_repository.rs`
- [ ] **4.5** Implementar carga recursiva de carpetas
- [ ] **4.6** Escribir tests de integracion con directorio temporal
- [ ] **4.7** Verificar manejo de errores de archivo no encontrado

### Fase 5: Use Cases (Dias 6-7)

- [ ] **5.1** Crear modulo `application/src/use_cases/mod.rs`
- [ ] **5.2** Implementar `CreateWorkspace`
- [ ] **5.3** Implementar `SaveCollection`
- [ ] **5.4** Implementar `LoadCollection`
- [ ] **5.5** Implementar `CreateRequest`
- [ ] **5.6** Implementar `UpdateRequest` con rename opcional
- [ ] **5.7** Escribir tests unitarios con mock de FileSystem

### Fase 6: UI Components (Dias 7-9)

- [ ] **6.1** Crear `ui/src/components/file_dialog.slint`
- [ ] **6.2** Crear `ui/src/components/collection_tree.slint`
- [ ] **6.3** Crear `ui/src/components/save_open_buttons.slint`
- [ ] **6.4** Implementar `CollectionState` en Rust para estado de UI
- [ ] **6.5** Conectar callbacks de Slint con use cases
- [ ] **6.6** Implementar seleccion de directorio nativo (rfd crate)
- [ ] **6.7** Agregar `rfd` a dependencias de ui

### Fase 7: Integracion (Dias 9-10)

- [ ] **7.1** Integrar sidebar de colecciones en layout principal
- [ ] **7.2** Conectar doble-click en request con apertura en editor
- [ ] **7.3** Implementar auto-save o confirmacion de cambios no guardados
- [ ] **7.4** Manejar errores de IO con mensajes de usuario
- [ ] **7.5** Probar flujo completo: crear workspace -> crear coleccion -> crear request -> guardar -> reabrir

### Fase 8: Testing y Polish (Dias 10-11)

- [ ] **8.1** Test E2E: crear workspace desde cero
- [ ] **8.2** Test E2E: abrir workspace existente
- [ ] **8.3** Test: verificar diffs de Git son limpios
- [ ] **8.4** Test: validar schema version mismatch
- [ ] **8.5** Documentar formato de archivos en README del proyecto
- [ ] **8.6** Cleanup: eliminar codigo dead, warnings

---

## Dependencias de Cargo

### domain/Cargo.toml

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4"] }
```

### application/Cargo.toml

```toml
[dependencies]
domain = { path = "../domain" }
async-trait = "0.1"
thiserror = "1.0"
tokio = { version = "1.0", features = ["fs"] }
```

### infrastructure/Cargo.toml

```toml
[dependencies]
domain = { path = "../domain" }
application = { path = "../application" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
thiserror = "1.0"
tokio = { version = "1.0", features = ["full"] }
```

### ui/Cargo.toml

```toml
[dependencies]
domain = { path = "../domain" }
application = { path = "../application" }
infrastructure = { path = "../infrastructure" }
slint = "1.0"
tokio = { version = "1.0", features = ["rt-multi-thread", "macros"] }
rfd = "0.12"  # Native file dialogs
```

---

## Criterios de Aceptacion

1. **Crear coleccion nueva**: Usuario puede crear una coleccion desde la UI, se genera la estructura de directorios correcta con `collection.json` valido.

2. **Guardar request**: Usuario puede crear y guardar un request, el archivo JSON tiene campos ordenados alfabeticamente y formato consistente.

3. **Reabrir sin perdida**: Usuario puede cerrar y reabrir el workspace, todos los datos se preservan exactamente.

4. **Diffs limpios**: Al modificar un solo campo de un request, `git diff` muestra solo esa linea cambiada (no reordenamiento de campos).

5. **Validacion de schema**: Si se intenta abrir un archivo con `schema_version` mayor al soportado, se muestra error claro al usuario.

6. **Estructura navegable**: La UI muestra el arbol de colecciones/carpetas/requests y permite navegar y expandir/colapsar.

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|--------|--------------|---------|------------|
| Cambios de schema rompen archivos existentes | Media | Alto | Implementar migraciones con `schema_version` desde el inicio |
| Colisiones de nombres de archivo | Baja | Medio | Funcion `slugify` + sufijos numericos si hay duplicados |
| Perdida de datos por error de IO | Baja | Alto | Guardar en archivo temporal primero, luego rename atomico |
| UI lenta con muchos requests | Media | Medio | Lazy loading de carpetas, virtualizacion de lista |
| Encoding incorrecto en Windows | Baja | Medio | Forzar UTF-8 sin BOM en todas las operaciones |

---

## Notas de Implementacion

1. **BTreeMap es clave**: Usar `BTreeMap` en lugar de `HashMap` para todos los campos `headers`, `query_params`, `variables` garantiza ordenamiento alfabetico sin esfuerzo extra.

2. **serde(skip_serializing_if)**: Usar este atributo para omitir campos opcionales que son `None` o colecciones vacias, manteniendo los archivos limpios.

3. **Rename atomico**: Al guardar, escribir a `.tmp` primero y luego `rename()` para evitar archivos corruptos si la app crashea mid-write.

4. **Tests con tempdir**: Usar `tempfile` crate para crear directorios temporales en tests, se limpian automaticamente.

5. **File dialog nativo**: El crate `rfd` (Rusty File Dialog) provee dialogs nativos en todas las plataformas sin dependencias pesadas.

---

## Referencias

- `02-file-format-spec.md` - Especificacion completa del formato de archivos
- Sprint 00 - Arquitectura base del workspace
- Sprint 01 - Modelos RequestSpec/ResponseSpec (runtime, no persistencia)
