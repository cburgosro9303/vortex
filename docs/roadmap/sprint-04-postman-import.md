# Sprint 04 - Postman Collection Import

**Objective:** Import Postman Collection v2.1 and Environment files into Vortex native format with full fidelity, providing clear feedback on any unsupported features.

**Duration:** 2 weeks
**Milestone:** M4
**Dependencies:** Sprint 01-03 (domain types, persistence, basic UI)

---

## Table of Contents

1. [Scope](#scope)
2. [Out of Scope](#out-of-scope)
3. [Postman Collection v2.1 Struct Definitions](#postman-collection-v21-struct-definitions)
4. [Postman Environment Struct Definitions](#postman-environment-struct-definitions)
5. [Mapping Logic](#mapping-logic)
6. [Import Use Case](#import-use-case)
7. [UI Components](#ui-components)
8. [Testing Strategy](#testing-strategy)
9. [Implementation Order](#implementation-order)
10. [Acceptance Criteria](#acceptance-criteria)
11. [Risks and Mitigations](#risks-and-mitigations)

---

## Scope

- Parse Postman Collection v2.1 JSON format
- Parse Postman Environment JSON format
- Convert to Vortex native format (as defined in `02-file-format-spec.md`)
- UI for import with preview and warnings
- Validation of malformed/oversized inputs
- Partial import support (skip unsupported items with warnings)

## Out of Scope

- Export to Postman format (future sprint)
- Postman Collection v1.0 format (deprecated)
- Pre-request/test scripts execution (JavaScript)
- Postman Flows, Monitors, Mock Servers
- Insomnia/OpenAPI import (future sprints)

---

## Postman Collection v2.1 Struct Definitions

All structs must be placed in a new module: `infrastructure/src/import/postman/types.rs`

### Root Collection Structure

```rust
//! Postman Collection v2.1 type definitions for import.
//!
//! These structs model the Postman Collection JSON schema.
//! Reference: https://schema.getpostman.com/json/collection/v2.1.0/collection.json
//!
//! Design principles:
//! - Use `#[serde(default)]` for optional fields
//! - Use `Option<T>` for fields that may be absent
//! - Use `#[serde(flatten)]` to capture unknown fields if needed
//! - Be tolerant: never fail on unknown fields

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root structure of a Postman Collection v2.1 file.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanCollection {
    /// Collection metadata (name, id, schema version).
    pub info: PostmanInfo,

    /// Items in the collection (requests and folders).
    #[serde(default)]
    pub item: Vec<PostmanItem>,

    /// Collection-level authentication (inherited by requests).
    #[serde(default)]
    pub auth: Option<PostmanAuth>,

    /// Collection-level variables.
    #[serde(default)]
    pub variable: Vec<PostmanVariable>,

    /// Pre-request script (not supported, will be skipped).
    #[serde(default)]
    pub event: Vec<PostmanEvent>,
}

/// Collection metadata.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanInfo {
    /// Postman-generated UUID.
    #[serde(rename = "_postman_id")]
    pub postman_id: Option<String>,

    /// Collection name.
    pub name: String,

    /// Collection description.
    #[serde(default)]
    pub description: Option<String>,

    /// Schema URL (should be v2.1.0).
    pub schema: String,

    /// Collection version.
    #[serde(default)]
    pub version: Option<String>,
}
```

### Item (Request or Folder)

```rust
/// An item in a Postman collection - can be a request or a folder.
///
/// Folders have nested `item` arrays; requests have a `request` object.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanItem {
    /// Item name.
    pub name: String,

    /// Unique identifier (optional in Postman).
    #[serde(default)]
    pub id: Option<String>,

    /// Item description.
    #[serde(default)]
    pub description: Option<String>,

    /// Nested items (if this is a folder).
    #[serde(default)]
    pub item: Vec<PostmanItem>,

    /// Request definition (if this is a request, not a folder).
    #[serde(default)]
    pub request: Option<PostmanRequest>,

    /// Expected responses (examples).
    #[serde(default)]
    pub response: Vec<PostmanResponse>,

    /// Item-level events (pre-request, test scripts).
    #[serde(default)]
    pub event: Vec<PostmanEvent>,

    /// Protocol profile behavior (optional).
    #[serde(default, rename = "protocolProfileBehavior")]
    pub protocol_profile_behavior: Option<serde_json::Value>,
}

impl PostmanItem {
    /// Returns true if this item is a folder (has nested items, no request).
    pub fn is_folder(&self) -> bool {
        !self.item.is_empty() && self.request.is_none()
    }

    /// Returns true if this item is a request.
    pub fn is_request(&self) -> bool {
        self.request.is_some()
    }
}
```

### Request Definition

```rust
/// A Postman request definition.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanRequest {
    /// HTTP method (GET, POST, etc.).
    pub method: String,

    /// Request URL (can be string or structured object).
    pub url: PostmanUrl,

    /// Request headers.
    #[serde(default)]
    pub header: Vec<PostmanHeader>,

    /// Request body.
    #[serde(default)]
    pub body: Option<PostmanBody>,

    /// Request-level authentication.
    #[serde(default)]
    pub auth: Option<PostmanAuth>,

    /// Request description.
    #[serde(default)]
    pub description: Option<String>,
}
```

### URL Structure

```rust
/// Postman URL - can be a simple string or a structured object.
///
/// Postman decomposes URLs into host, path, query, etc.
/// We need to handle both the `raw` string and the structured format.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PostmanUrl {
    /// Simple string URL.
    Simple(String),

    /// Structured URL with components.
    Structured(PostmanUrlStructured),
}

impl PostmanUrl {
    /// Converts to a single URL string, preferring `raw` if available.
    pub fn to_url_string(&self) -> String {
        match self {
            PostmanUrl::Simple(s) => s.clone(),
            PostmanUrl::Structured(structured) => structured.to_url_string(),
        }
    }

    /// Extracts query parameters as key-value pairs.
    pub fn query_params(&self) -> Vec<(String, String)> {
        match self {
            PostmanUrl::Simple(_) => vec![], // Query params are in the URL string
            PostmanUrl::Structured(s) => s.query_params(),
        }
    }
}

/// Structured URL representation in Postman.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanUrlStructured {
    /// Raw URL string (usually complete).
    #[serde(default)]
    pub raw: Option<String>,

    /// Protocol (http, https).
    #[serde(default)]
    pub protocol: Option<String>,

    /// Host segments (e.g., ["api", "example", "com"]).
    #[serde(default)]
    pub host: PostmanHostOrString,

    /// Port number.
    #[serde(default)]
    pub port: Option<String>,

    /// Path segments (e.g., ["users", "{{user_id}}"]).
    #[serde(default)]
    pub path: PostmanPathOrString,

    /// Query parameters.
    #[serde(default)]
    pub query: Vec<PostmanQueryParam>,

    /// Hash/fragment.
    #[serde(default)]
    pub hash: Option<String>,

    /// URL variables (path variables like :id).
    #[serde(default)]
    pub variable: Vec<PostmanVariable>,
}

impl PostmanUrlStructured {
    /// Reconstructs URL string from components.
    /// Prefers `raw` if available, otherwise builds from parts.
    pub fn to_url_string(&self) -> String {
        // Prefer raw URL if available
        if let Some(raw) = &self.raw {
            return raw.clone();
        }

        // Build from components
        let mut url = String::new();

        // Protocol
        if let Some(protocol) = &self.protocol {
            url.push_str(protocol);
            url.push_str("://");
        }

        // Host
        url.push_str(&self.host.to_string());

        // Port
        if let Some(port) = &self.port {
            url.push(':');
            url.push_str(port);
        }

        // Path
        let path = self.path.to_string();
        if !path.is_empty() && !path.starts_with('/') {
            url.push('/');
        }
        url.push_str(&path);

        url
    }

    /// Extracts query parameters.
    pub fn query_params(&self) -> Vec<(String, String)> {
        self.query
            .iter()
            .filter(|q| q.disabled != Some(true))
            .map(|q| (q.key.clone(), q.value.clone().unwrap_or_default()))
            .collect()
    }
}

/// Host can be a string or array of strings.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(untagged)]
pub enum PostmanHostOrString {
    #[default]
    Empty,
    String(String),
    Array(Vec<String>),
}

impl PostmanHostOrString {
    pub fn to_string(&self) -> String {
        match self {
            PostmanHostOrString::Empty => String::new(),
            PostmanHostOrString::String(s) => s.clone(),
            PostmanHostOrString::Array(arr) => arr.join("."),
        }
    }
}

/// Path can be a string or array of strings.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(untagged)]
pub enum PostmanPathOrString {
    #[default]
    Empty,
    String(String),
    Array(Vec<PostmanPathSegment>),
}

impl PostmanPathOrString {
    pub fn to_string(&self) -> String {
        match self {
            PostmanPathOrString::Empty => String::new(),
            PostmanPathOrString::String(s) => s.clone(),
            PostmanPathOrString::Array(arr) => arr
                .iter()
                .map(|seg| seg.to_string())
                .collect::<Vec<_>>()
                .join("/"),
        }
    }
}

/// Path segment can be a string or an object with value.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PostmanPathSegment {
    String(String),
    Object { value: String },
}

impl PostmanPathSegment {
    pub fn to_string(&self) -> String {
        match self {
            PostmanPathSegment::String(s) => s.clone(),
            PostmanPathSegment::Object { value } => value.clone(),
        }
    }
}

/// Query parameter in Postman URL.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanQueryParam {
    /// Parameter key.
    pub key: String,

    /// Parameter value.
    #[serde(default)]
    pub value: Option<String>,

    /// Description.
    #[serde(default)]
    pub description: Option<String>,

    /// Whether this param is disabled.
    #[serde(default)]
    pub disabled: Option<bool>,
}
```

### Headers

```rust
/// A single HTTP header.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanHeader {
    /// Header name.
    pub key: String,

    /// Header value.
    pub value: String,

    /// Header description.
    #[serde(default)]
    pub description: Option<String>,

    /// Header type (usually "text").
    #[serde(default, rename = "type")]
    pub header_type: Option<String>,

    /// Whether this header is disabled.
    #[serde(default)]
    pub disabled: Option<bool>,
}
```

### Request Body

```rust
/// Request body in Postman.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanBody {
    /// Body mode: raw, urlencoded, formdata, file, graphql.
    pub mode: String,

    /// Raw body content (for mode=raw).
    #[serde(default)]
    pub raw: Option<String>,

    /// Form URL-encoded data (for mode=urlencoded).
    #[serde(default)]
    pub urlencoded: Vec<PostmanFormParam>,

    /// Multipart form data (for mode=formdata).
    #[serde(default)]
    pub formdata: Vec<PostmanFormParam>,

    /// File path (for mode=file).
    #[serde(default)]
    pub file: Option<PostmanFileBody>,

    /// GraphQL query (for mode=graphql).
    #[serde(default)]
    pub graphql: Option<PostmanGraphQLBody>,

    /// Options for raw body (language hint).
    #[serde(default)]
    pub options: Option<PostmanBodyOptions>,
}

/// Form parameter (used for urlencoded and formdata).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanFormParam {
    /// Parameter key.
    pub key: String,

    /// Parameter value (for text type).
    #[serde(default)]
    pub value: Option<String>,

    /// File source path (for file type).
    #[serde(default)]
    pub src: Option<String>,

    /// Parameter description.
    #[serde(default)]
    pub description: Option<String>,

    /// Parameter type: "text" or "file".
    #[serde(default, rename = "type")]
    pub param_type: Option<String>,

    /// Content type for this parameter.
    #[serde(default, rename = "contentType")]
    pub content_type: Option<String>,

    /// Whether this parameter is disabled.
    #[serde(default)]
    pub disabled: Option<bool>,
}

/// File body reference.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanFileBody {
    /// File source path.
    #[serde(default)]
    pub src: Option<String>,

    /// File content (base64 or raw, rarely used).
    #[serde(default)]
    pub content: Option<String>,
}

/// GraphQL body.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanGraphQLBody {
    /// GraphQL query string.
    #[serde(default)]
    pub query: Option<String>,

    /// GraphQL variables as JSON string.
    #[serde(default)]
    pub variables: Option<String>,
}

/// Body options (raw body language).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanBodyOptions {
    /// Raw body options.
    #[serde(default)]
    pub raw: Option<PostmanRawOptions>,
}

/// Raw body language options.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanRawOptions {
    /// Language: json, xml, html, text, javascript.
    #[serde(default)]
    pub language: Option<String>,
}
```

### Authentication

```rust
/// Authentication configuration in Postman.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanAuth {
    /// Auth type: noauth, basic, bearer, apikey, oauth2, etc.
    #[serde(rename = "type")]
    pub auth_type: String,

    /// Basic auth parameters.
    #[serde(default)]
    pub basic: Vec<PostmanAuthParam>,

    /// Bearer token parameters.
    #[serde(default)]
    pub bearer: Vec<PostmanAuthParam>,

    /// API key parameters.
    #[serde(default)]
    pub apikey: Vec<PostmanAuthParam>,

    /// OAuth2 parameters.
    #[serde(default)]
    pub oauth2: Vec<PostmanAuthParam>,

    /// AWS Signature parameters.
    #[serde(default)]
    pub awsv4: Vec<PostmanAuthParam>,

    /// Digest auth parameters.
    #[serde(default)]
    pub digest: Vec<PostmanAuthParam>,

    /// NTLM auth parameters.
    #[serde(default)]
    pub ntlm: Vec<PostmanAuthParam>,
}

/// A single auth parameter (key-value pair).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanAuthParam {
    /// Parameter key (e.g., "username", "password", "token").
    pub key: String,

    /// Parameter value.
    pub value: serde_json::Value,

    /// Parameter type (usually "string").
    #[serde(default, rename = "type")]
    pub param_type: Option<String>,
}

impl PostmanAuth {
    /// Helper to get a parameter value by key from any auth type.
    pub fn get_param(&self, key: &str) -> Option<String> {
        let params = match self.auth_type.as_str() {
            "basic" => &self.basic,
            "bearer" => &self.bearer,
            "apikey" => &self.apikey,
            "oauth2" => &self.oauth2,
            "awsv4" => &self.awsv4,
            "digest" => &self.digest,
            "ntlm" => &self.ntlm,
            _ => return None,
        };

        params.iter().find(|p| p.key == key).and_then(|p| {
            match &p.value {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                serde_json::Value::Bool(b) => Some(b.to_string()),
                _ => None,
            }
        })
    }
}
```

### Variables and Events

```rust
/// A variable in Postman (collection or environment level).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanVariable {
    /// Variable key/name.
    pub key: String,

    /// Variable value.
    #[serde(default)]
    pub value: Option<serde_json::Value>,

    /// Variable description.
    #[serde(default)]
    pub description: Option<String>,

    /// Variable type.
    #[serde(default, rename = "type")]
    pub var_type: Option<String>,

    /// Whether this variable is disabled.
    #[serde(default)]
    pub disabled: Option<bool>,
}

impl PostmanVariable {
    /// Gets the value as a string.
    pub fn value_as_string(&self) -> String {
        match &self.value {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Number(n)) => n.to_string(),
            Some(serde_json::Value::Bool(b)) => b.to_string(),
            Some(serde_json::Value::Null) => String::new(),
            Some(v) => v.to_string(),
            None => String::new(),
        }
    }
}

/// Event (script) in Postman - pre-request or test.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanEvent {
    /// Event type: "prerequest" or "test".
    pub listen: String,

    /// Script definition.
    #[serde(default)]
    pub script: Option<PostmanScript>,
}

/// Script definition.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanScript {
    /// Script ID.
    #[serde(default)]
    pub id: Option<String>,

    /// Script type (usually "text/javascript").
    #[serde(default, rename = "type")]
    pub script_type: Option<String>,

    /// Script content as array of lines.
    #[serde(default)]
    pub exec: Vec<String>,
}

/// Saved response example.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanResponse {
    /// Response name.
    pub name: String,

    /// Original request that generated this response.
    #[serde(default, rename = "originalRequest")]
    pub original_request: Option<PostmanRequest>,

    /// HTTP status code.
    #[serde(default)]
    pub code: Option<u16>,

    /// Status text (e.g., "OK").
    #[serde(default)]
    pub status: Option<String>,

    /// Response headers.
    #[serde(default)]
    pub header: Vec<PostmanHeader>,

    /// Response body.
    #[serde(default)]
    pub body: Option<String>,
}
```

---

## Postman Environment Struct Definitions

Place in: `infrastructure/src/import/postman/environment_types.rs`

```rust
//! Postman Environment type definitions for import.

use serde::{Deserialize, Serialize};

/// Root structure of a Postman Environment file.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanEnvironment {
    /// Environment ID.
    pub id: String,

    /// Environment name.
    pub name: String,

    /// Variables in this environment.
    #[serde(default)]
    pub values: Vec<PostmanEnvVariable>,

    /// Postman-specific metadata.
    #[serde(default, rename = "_postman_variable_scope")]
    pub scope: Option<String>,

    /// Whether this is exported from cloud.
    #[serde(default, rename = "_postman_exported_at")]
    pub exported_at: Option<String>,

    /// Export source.
    #[serde(default, rename = "_postman_exported_using")]
    pub exported_using: Option<String>,
}

/// A variable in a Postman environment.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostmanEnvVariable {
    /// Variable key.
    pub key: String,

    /// Variable value.
    #[serde(default)]
    pub value: String,

    /// Whether variable is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Variable type (secret, default, etc.).
    #[serde(default, rename = "type")]
    pub var_type: Option<String>,
}

fn default_true() -> bool {
    true
}
```

---

## Mapping Logic

Place in: `infrastructure/src/import/postman/mapper.rs`

### PostmanItem to Vortex Request/Folder

```rust
use crate::import::postman::types::*;
use domain::{
    Request, RequestId, Folder, FolderId, HttpMethod, Auth, Body,
    FormField, FormFieldType, Collection, CollectionId, Environment,
    EnvironmentId, Variable, VariableValue,
};
use uuid::Uuid;

/// Result of mapping a Postman item.
pub enum MappedItem {
    Request(Request),
    Folder(Folder, Vec<MappedItem>),
}

/// Warnings generated during import.
#[derive(Debug, Clone)]
pub struct ImportWarning {
    pub path: String,
    pub message: String,
    pub severity: WarningSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningSeverity {
    Info,
    Warning,
    Error,
}

/// Maps a PostmanItem to Vortex domain types.
pub fn map_postman_item(
    item: &PostmanItem,
    parent_path: &str,
    depth: usize,
    max_depth: usize,
    warnings: &mut Vec<ImportWarning>,
) -> Option<MappedItem> {
    let current_path = format!("{}/{}", parent_path, item.name);

    // Check depth limit
    if depth > max_depth {
        warnings.push(ImportWarning {
            path: current_path.clone(),
            message: format!("Skipped: exceeds maximum nesting depth of {}", max_depth),
            severity: WarningSeverity::Warning,
        });
        return None;
    }

    // Warn about scripts (unsupported)
    if !item.event.is_empty() {
        let has_prerequest = item.event.iter().any(|e| e.listen == "prerequest");
        let has_test = item.event.iter().any(|e| e.listen == "test");

        if has_prerequest {
            warnings.push(ImportWarning {
                path: current_path.clone(),
                message: "Pre-request scripts are not supported and will be skipped".to_string(),
                severity: WarningSeverity::Info,
            });
        }
        if has_test {
            warnings.push(ImportWarning {
                path: current_path.clone(),
                message: "Test scripts are not supported and will be skipped".to_string(),
                severity: WarningSeverity::Info,
            });
        }
    }

    if item.is_folder() {
        // Map as folder
        let folder = Folder {
            id: FolderId::new(Uuid::new_v4()),
            name: item.name.clone(),
            description: item.description.clone(),
            auth: None, // Could inherit from parent
            order: vec![],
        };

        let children: Vec<MappedItem> = item
            .item
            .iter()
            .filter_map(|child| {
                map_postman_item(child, &current_path, depth + 1, max_depth, warnings)
            })
            .collect();

        Some(MappedItem::Folder(folder, children))
    } else if let Some(req) = &item.request {
        // Map as request
        let request = map_postman_request(req, &item.name, &current_path, warnings);
        Some(MappedItem::Request(request))
    } else {
        // Empty item (no request, no children)
        warnings.push(ImportWarning {
            path: current_path,
            message: "Empty item skipped".to_string(),
            severity: WarningSeverity::Info,
        });
        None
    }
}

fn map_postman_request(
    req: &PostmanRequest,
    name: &str,
    path: &str,
    warnings: &mut Vec<ImportWarning>,
) -> Request {
    Request {
        id: RequestId::new(Uuid::new_v4()),
        name: name.to_string(),
        schema_version: 1,
        method: map_http_method(&req.method),
        url: req.url.to_url_string(),
        headers: map_headers(&req.header),
        query_params: map_query_params(&req.url),
        body: map_body(&req.body, path, warnings),
        auth: map_auth(&req.auth, path, warnings),
        settings: Default::default(),
        tests: vec![], // Scripts not supported
    }
}
```

### URL Mapping

```rust
/// Maps Postman URL to a single URL string.
/// Postman stores URLs in a decomposed format; we reassemble them.
fn map_url(url: &PostmanUrl) -> String {
    url.to_url_string()
}

/// Extracts query parameters from Postman URL.
/// Returns HashMap for Vortex format.
fn map_query_params(url: &PostmanUrl) -> HashMap<String, String> {
    url.query_params()
        .into_iter()
        .collect()
}
```

### Headers Mapping

```rust
/// Maps Postman headers to Vortex format.
/// Filters out disabled headers.
fn map_headers(headers: &[PostmanHeader]) -> HashMap<String, String> {
    headers
        .iter()
        .filter(|h| h.disabled != Some(true))
        .map(|h| (h.key.clone(), h.value.clone()))
        .collect()
}
```

### Body Mapping

```rust
/// Maps Postman body to Vortex Body type.
fn map_body(
    body: &Option<PostmanBody>,
    path: &str,
    warnings: &mut Vec<ImportWarning>,
) -> Option<Body> {
    let body = body.as_ref()?;

    match body.mode.as_str() {
        "raw" => {
            let content = body.raw.clone().unwrap_or_default();
            let language = body
                .options
                .as_ref()
                .and_then(|o| o.raw.as_ref())
                .and_then(|r| r.language.as_ref())
                .map(|s| s.as_str());

            match language {
                Some("json") => {
                    // Try to parse as JSON for structured storage
                    match serde_json::from_str::<serde_json::Value>(&content) {
                        Ok(json_value) => Some(Body::Json { content: json_value }),
                        Err(_) => {
                            warnings.push(ImportWarning {
                                path: path.to_string(),
                                message: "Body marked as JSON but is invalid JSON; importing as text".to_string(),
                                severity: WarningSeverity::Warning,
                            });
                            Some(Body::Text { content })
                        }
                    }
                }
                Some("xml") | Some("html") | Some("text") | _ => {
                    Some(Body::Text { content })
                }
            }
        }

        "urlencoded" => {
            let fields: HashMap<String, String> = body
                .urlencoded
                .iter()
                .filter(|p| p.disabled != Some(true))
                .map(|p| (p.key.clone(), p.value.clone().unwrap_or_default()))
                .collect();

            Some(Body::FormUrlEncoded { fields })
        }

        "formdata" => {
            let fields: Vec<FormField> = body
                .formdata
                .iter()
                .filter(|p| p.disabled != Some(true))
                .map(|p| {
                    let is_file = p.param_type.as_deref() == Some("file");
                    if is_file {
                        FormField {
                            name: p.key.clone(),
                            field_type: FormFieldType::File,
                            value: p.src.clone().unwrap_or_default(),
                        }
                    } else {
                        FormField {
                            name: p.key.clone(),
                            field_type: FormFieldType::Text,
                            value: p.value.clone().unwrap_or_default(),
                        }
                    }
                })
                .collect();

            Some(Body::FormData { fields })
        }

        "file" => {
            let file_path = body
                .file
                .as_ref()
                .and_then(|f| f.src.clone())
                .unwrap_or_default();

            Some(Body::Binary { path: file_path })
        }

        "graphql" => {
            if let Some(gql) = &body.graphql {
                let query = gql.query.clone().unwrap_or_default();
                let variables: Option<serde_json::Value> = gql
                    .variables
                    .as_ref()
                    .and_then(|v| serde_json::from_str(v).ok());

                Some(Body::GraphQL {
                    query,
                    variables: variables.unwrap_or(serde_json::Value::Null),
                })
            } else {
                None
            }
        }

        other => {
            warnings.push(ImportWarning {
                path: path.to_string(),
                message: format!("Unknown body mode '{}'; body skipped", other),
                severity: WarningSeverity::Warning,
            });
            None
        }
    }
}
```

### Auth Mapping

```rust
/// Maps Postman auth to Vortex Auth type.
fn map_auth(
    auth: &Option<PostmanAuth>,
    path: &str,
    warnings: &mut Vec<ImportWarning>,
) -> Option<Auth> {
    let auth = auth.as_ref()?;

    match auth.auth_type.as_str() {
        "noauth" => None,

        "basic" => {
            let username = auth.get_param("username").unwrap_or_default();
            let password = auth.get_param("password").unwrap_or_default();
            Some(Auth::Basic { username, password })
        }

        "bearer" => {
            let token = auth.get_param("token").unwrap_or_default();
            Some(Auth::Bearer { token })
        }

        "apikey" => {
            let key = auth.get_param("key").unwrap_or_default();
            let value = auth.get_param("value").unwrap_or_default();
            let location = auth.get_param("in").unwrap_or_else(|| "header".to_string());
            Some(Auth::ApiKey { key, value, location })
        }

        "oauth2" => {
            // OAuth2 has many grant types; try to detect which one
            let grant_type = auth.get_param("grant_type");
            let access_token = auth.get_param("accessToken");
            let token_url = auth.get_param("accessTokenUrl");
            let auth_url = auth.get_param("authUrl");
            let client_id = auth.get_param("clientId").unwrap_or_default();
            let client_secret = auth.get_param("clientSecret").unwrap_or_default();
            let scope = auth.get_param("scope").unwrap_or_default();

            match grant_type.as_deref() {
                Some("client_credentials") => {
                    Some(Auth::OAuth2ClientCredentials {
                        token_url: token_url.unwrap_or_default(),
                        client_id,
                        client_secret,
                        scope,
                    })
                }
                Some("authorization_code") | Some("authorization_code_with_pkce") => {
                    Some(Auth::OAuth2AuthCode {
                        auth_url: auth_url.unwrap_or_default(),
                        token_url: token_url.unwrap_or_default(),
                        client_id,
                        client_secret,
                        redirect_uri: auth.get_param("redirectUri").unwrap_or_default(),
                        scope,
                    })
                }
                _ => {
                    // If there's an access token, just use bearer
                    if let Some(token) = access_token {
                        Some(Auth::Bearer { token })
                    } else {
                        warnings.push(ImportWarning {
                            path: path.to_string(),
                            message: format!(
                                "OAuth2 grant type '{}' not fully supported; skipping auth",
                                grant_type.unwrap_or_default()
                            ),
                            severity: WarningSeverity::Warning,
                        });
                        None
                    }
                }
            }
        }

        "awsv4" | "digest" | "ntlm" | "hawk" | "edgegrid" => {
            warnings.push(ImportWarning {
                path: path.to_string(),
                message: format!(
                    "Auth type '{}' is not supported; skipping auth",
                    auth.auth_type
                ),
                severity: WarningSeverity::Warning,
            });
            None
        }

        other => {
            warnings.push(ImportWarning {
                path: path.to_string(),
                message: format!("Unknown auth type '{}'; skipping auth", other),
                severity: WarningSeverity::Warning,
            });
            None
        }
    }
}
```

### HTTP Method Mapping

```rust
/// Maps HTTP method string to enum.
fn map_http_method(method: &str) -> HttpMethod {
    match method.to_uppercase().as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "HEAD" => HttpMethod::Head,
        "OPTIONS" => HttpMethod::Options,
        "TRACE" => HttpMethod::Trace,
        _ => HttpMethod::Get, // Default fallback
    }
}
```

### Variable Mapping

```rust
/// Maps Postman collection variables to Vortex format.
pub fn map_collection_variables(
    variables: &[PostmanVariable],
) -> HashMap<String, String> {
    variables
        .iter()
        .filter(|v| v.disabled != Some(true))
        .map(|v| (v.key.clone(), v.value_as_string()))
        .collect()
}

/// Maps Postman environment to Vortex Environment.
pub fn map_postman_environment(
    env: &PostmanEnvironment,
    warnings: &mut Vec<ImportWarning>,
) -> Environment {
    let variables: HashMap<String, VariableValue> = env
        .values
        .iter()
        .filter(|v| v.enabled)
        .map(|v| {
            let is_secret = v.var_type.as_deref() == Some("secret");
            (
                v.key.clone(),
                VariableValue {
                    value: v.value.clone(),
                    secret: is_secret,
                },
            )
        })
        .collect();

    Environment {
        id: EnvironmentId::new(Uuid::new_v4()),
        name: env.name.clone(),
        schema_version: 1,
        variables,
    }
}
```

---

## Import Use Case

Place in: `application/src/use_cases/import_postman.rs`

### Trait Definition

```rust
use async_trait::async_trait;
use domain::{Collection, Environment};
use std::path::Path;

/// Result of a Postman import operation.
#[derive(Debug)]
pub struct ImportResult {
    /// Successfully imported collection.
    pub collection: Option<Collection>,

    /// Successfully imported environments.
    pub environments: Vec<Environment>,

    /// Warnings generated during import.
    pub warnings: Vec<ImportWarning>,

    /// Critical errors that prevented full import.
    pub errors: Vec<ImportError>,

    /// Statistics about the import.
    pub stats: ImportStats,
}

#[derive(Debug, Default)]
pub struct ImportStats {
    pub requests_imported: usize,
    pub requests_skipped: usize,
    pub folders_imported: usize,
    pub folders_skipped: usize,
    pub variables_imported: usize,
    pub environments_imported: usize,
}

#[derive(Debug)]
pub struct ImportError {
    pub message: String,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

/// Configuration for import validation.
#[derive(Debug, Clone)]
pub struct ImportConfig {
    /// Maximum file size in bytes (default: 10 MB).
    pub max_file_size: usize,

    /// Maximum nesting depth for folders (default: 10).
    pub max_depth: usize,

    /// Maximum number of items (requests + folders) (default: 1000).
    pub max_items: usize,

    /// Whether to continue on non-critical errors.
    pub partial_import: bool,
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10 MB
            max_depth: 10,
            max_items: 1000,
            partial_import: true,
        }
    }
}

/// Use case for importing Postman collections.
#[async_trait]
pub trait ImportPostmanCollection: Send + Sync {
    /// Imports a Postman collection from a file path.
    async fn import_collection(
        &self,
        path: &Path,
        config: &ImportConfig,
    ) -> Result<ImportResult, ImportError>;

    /// Imports a Postman environment from a file path.
    async fn import_environment(
        &self,
        path: &Path,
        config: &ImportConfig,
    ) -> Result<ImportResult, ImportError>;

    /// Validates a file before import (checks size, basic JSON validity).
    async fn validate_file(
        &self,
        path: &Path,
        config: &ImportConfig,
    ) -> Result<ValidationResult, ImportError>;

    /// Previews what would be imported without actually importing.
    async fn preview_import(
        &self,
        path: &Path,
        config: &ImportConfig,
    ) -> Result<ImportPreview, ImportError>;
}

#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub file_size: usize,
    pub detected_format: DetectedFormat,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedFormat {
    PostmanCollectionV21,
    PostmanCollectionV20,
    PostmanEnvironment,
    Unknown,
}

#[derive(Debug)]
pub struct ImportPreview {
    pub collection_name: Option<String>,
    pub environment_name: Option<String>,
    pub request_count: usize,
    pub folder_count: usize,
    pub variable_count: usize,
    pub warnings: Vec<ImportWarning>,
    pub detected_format: DetectedFormat,
}
```

### Implementation

```rust
use crate::use_cases::import_postman::*;
use infrastructure::import::postman::{
    mapper::{map_postman_item, map_collection_variables, map_postman_environment, MappedItem},
    types::{PostmanCollection, PostmanInfo},
    environment_types::PostmanEnvironment,
};
use domain::{Collection, CollectionId, Request, Folder};
use async_trait::async_trait;
use std::path::Path;
use tokio::fs;
use uuid::Uuid;

pub struct PostmanImporter {
    // Could hold repository references for persistence
}

impl PostmanImporter {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ImportPostmanCollection for PostmanImporter {
    async fn import_collection(
        &self,
        path: &Path,
        config: &ImportConfig,
    ) -> Result<ImportResult, ImportError> {
        // 1. Validate file
        let validation = self.validate_file(path, config).await?;
        if !validation.is_valid {
            return Err(ImportError {
                message: format!("Validation failed: {}", validation.issues.join(", ")),
                source: None,
            });
        }

        // 2. Read and parse JSON
        let content = fs::read_to_string(path).await.map_err(|e| ImportError {
            message: format!("Failed to read file: {}", e),
            source: Some(Box::new(e)),
        })?;

        let postman_collection: PostmanCollection =
            serde_json::from_str(&content).map_err(|e| ImportError {
                message: format!("Invalid JSON: {}", e),
                source: Some(Box::new(e)),
            })?;

        // 3. Map to Vortex types
        let mut warnings = Vec::new();
        let mut stats = ImportStats::default();

        let mapped_items: Vec<MappedItem> = postman_collection
            .item
            .iter()
            .filter_map(|item| {
                map_postman_item(item, "", 0, config.max_depth, &mut warnings)
            })
            .collect();

        // 4. Build collection
        let (requests, folders) = flatten_mapped_items(mapped_items, &mut stats);

        // Check item limit
        let total_items = stats.requests_imported + stats.folders_imported;
        if total_items > config.max_items {
            return Err(ImportError {
                message: format!(
                    "Collection has {} items, exceeds limit of {}",
                    total_items, config.max_items
                ),
                source: None,
            });
        }

        let collection = Collection {
            id: CollectionId::new(Uuid::new_v4()),
            name: postman_collection.info.name.clone(),
            schema_version: 1,
            description: postman_collection.info.description.clone(),
            auth: None, // TODO: map collection-level auth
            variables: map_collection_variables(&postman_collection.variable),
            requests,
            folders,
        };

        stats.variables_imported = postman_collection.variable.len();

        Ok(ImportResult {
            collection: Some(collection),
            environments: vec![],
            warnings,
            errors: vec![],
            stats,
        })
    }

    async fn import_environment(
        &self,
        path: &Path,
        config: &ImportConfig,
    ) -> Result<ImportResult, ImportError> {
        // Validate file size
        let metadata = fs::metadata(path).await.map_err(|e| ImportError {
            message: format!("Failed to read file metadata: {}", e),
            source: Some(Box::new(e)),
        })?;

        if metadata.len() as usize > config.max_file_size {
            return Err(ImportError {
                message: format!(
                    "File size {} exceeds limit of {} bytes",
                    metadata.len(),
                    config.max_file_size
                ),
                source: None,
            });
        }

        // Read and parse
        let content = fs::read_to_string(path).await.map_err(|e| ImportError {
            message: format!("Failed to read file: {}", e),
            source: Some(Box::new(e)),
        })?;

        let postman_env: PostmanEnvironment =
            serde_json::from_str(&content).map_err(|e| ImportError {
                message: format!("Invalid JSON: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut warnings = Vec::new();
        let environment = map_postman_environment(&postman_env, &mut warnings);

        let stats = ImportStats {
            environments_imported: 1,
            variables_imported: environment.variables.len(),
            ..Default::default()
        };

        Ok(ImportResult {
            collection: None,
            environments: vec![environment],
            warnings,
            errors: vec![],
            stats,
        })
    }

    async fn validate_file(
        &self,
        path: &Path,
        config: &ImportConfig,
    ) -> Result<ValidationResult, ImportError> {
        let mut issues = Vec::new();

        // Check file exists
        if !path.exists() {
            return Ok(ValidationResult {
                is_valid: false,
                file_size: 0,
                detected_format: DetectedFormat::Unknown,
                issues: vec!["File does not exist".to_string()],
            });
        }

        // Check file size
        let metadata = fs::metadata(path).await.map_err(|e| ImportError {
            message: format!("Failed to read file metadata: {}", e),
            source: Some(Box::new(e)),
        })?;

        let file_size = metadata.len() as usize;
        if file_size > config.max_file_size {
            issues.push(format!(
                "File size {} exceeds limit of {} bytes",
                file_size, config.max_file_size
            ));
        }

        // Check JSON validity and detect format
        let content = fs::read_to_string(path).await.map_err(|e| ImportError {
            message: format!("Failed to read file: {}", e),
            source: Some(Box::new(e)),
        })?;

        let detected_format = detect_format(&content);

        if detected_format == DetectedFormat::Unknown {
            issues.push("Unable to detect Postman format".to_string());
        }

        Ok(ValidationResult {
            is_valid: issues.is_empty(),
            file_size,
            detected_format,
            issues,
        })
    }

    async fn preview_import(
        &self,
        path: &Path,
        config: &ImportConfig,
    ) -> Result<ImportPreview, ImportError> {
        let content = fs::read_to_string(path).await.map_err(|e| ImportError {
            message: format!("Failed to read file: {}", e),
            source: Some(Box::new(e)),
        })?;

        let detected_format = detect_format(&content);

        match detected_format {
            DetectedFormat::PostmanCollectionV21 | DetectedFormat::PostmanCollectionV20 => {
                let collection: PostmanCollection =
                    serde_json::from_str(&content).map_err(|e| ImportError {
                        message: format!("Invalid JSON: {}", e),
                        source: Some(Box::new(e)),
                    })?;

                let mut warnings = Vec::new();
                let (request_count, folder_count) =
                    count_items(&collection.item, 0, config.max_depth, &mut warnings);

                Ok(ImportPreview {
                    collection_name: Some(collection.info.name),
                    environment_name: None,
                    request_count,
                    folder_count,
                    variable_count: collection.variable.len(),
                    warnings,
                    detected_format,
                })
            }
            DetectedFormat::PostmanEnvironment => {
                let env: PostmanEnvironment =
                    serde_json::from_str(&content).map_err(|e| ImportError {
                        message: format!("Invalid JSON: {}", e),
                        source: Some(Box::new(e)),
                    })?;

                Ok(ImportPreview {
                    collection_name: None,
                    environment_name: Some(env.name),
                    request_count: 0,
                    folder_count: 0,
                    variable_count: env.values.len(),
                    warnings: vec![],
                    detected_format,
                })
            }
            DetectedFormat::Unknown => Err(ImportError {
                message: "Unable to detect Postman format".to_string(),
                source: None,
            }),
        }
    }
}

/// Detects the Postman format from JSON content.
fn detect_format(content: &str) -> DetectedFormat {
    // Try to parse as generic JSON first
    let json: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return DetectedFormat::Unknown,
    };

    // Check for collection (has "info" with "schema")
    if let Some(info) = json.get("info") {
        if let Some(schema) = info.get("schema").and_then(|s| s.as_str()) {
            if schema.contains("v2.1") {
                return DetectedFormat::PostmanCollectionV21;
            } else if schema.contains("v2.0") {
                return DetectedFormat::PostmanCollectionV20;
            }
        }
    }

    // Check for environment (has "values" array and no "info")
    if json.get("values").is_some() && json.get("info").is_none() {
        return DetectedFormat::PostmanEnvironment;
    }

    DetectedFormat::Unknown
}

/// Counts items recursively for preview.
fn count_items(
    items: &[PostmanItem],
    depth: usize,
    max_depth: usize,
    warnings: &mut Vec<ImportWarning>,
) -> (usize, usize) {
    let mut requests = 0;
    let mut folders = 0;

    for item in items {
        if depth > max_depth {
            warnings.push(ImportWarning {
                path: item.name.clone(),
                message: "Exceeds max depth".to_string(),
                severity: WarningSeverity::Warning,
            });
            continue;
        }

        if item.is_folder() {
            folders += 1;
            let (child_requests, child_folders) =
                count_items(&item.item, depth + 1, max_depth, warnings);
            requests += child_requests;
            folders += child_folders;
        } else if item.is_request() {
            requests += 1;
        }
    }

    (requests, folders)
}

/// Flattens mapped items into separate request and folder lists.
fn flatten_mapped_items(
    items: Vec<MappedItem>,
    stats: &mut ImportStats,
) -> (Vec<Request>, Vec<Folder>) {
    let mut requests = Vec::new();
    let mut folders = Vec::new();

    for item in items {
        match item {
            MappedItem::Request(req) => {
                stats.requests_imported += 1;
                requests.push(req);
            }
            MappedItem::Folder(folder, children) => {
                stats.folders_imported += 1;
                let (child_requests, child_folders) = flatten_mapped_items(children, stats);
                requests.extend(child_requests);
                folders.push(folder);
                folders.extend(child_folders);
            }
        }
    }

    (requests, folders)
}
```

---

## UI Components

Place Slint components in: `ui/src/components/import/`

### Import Dialog (import_dialog.slint)

```slint
// Import dialog with file picker, format selector, preview, and warnings.

import { Button, ComboBox, ListView, ProgressIndicator, StandardButton } from "std-widgets.slint";

export enum ImportFormat {
    PostmanV21,
    PostmanEnvironment,
    // Future: Insomnia, OpenAPI
}

export enum ImportState {
    Idle,
    Validating,
    Previewing,
    Importing,
    Complete,
    Error,
}

export struct ImportWarningItem {
    path: string,
    message: string,
    severity: string, // "info", "warning", "error"
}

export struct ImportPreviewData {
    collection-name: string,
    environment-name: string,
    request-count: int,
    folder-count: int,
    variable-count: int,
}

export component ImportDialog inherits Dialog {
    title: "Import Collection";
    min-width: 600px;
    min-height: 500px;

    // Properties
    in-out property <string> selected-file: "";
    in-out property <ImportFormat> selected-format: ImportFormat.PostmanV21;
    in-out property <ImportState> state: ImportState.Idle;
    in-out property <[ImportWarningItem]> warnings: [];
    in-out property <ImportPreviewData> preview;
    in-out property <string> error-message: "";
    in-out property <float> progress: 0.0;

    // Callbacks
    callback browse-file();
    callback format-changed(ImportFormat);
    callback start-import();
    callback cancel-import();

    VerticalLayout {
        padding: 20px;
        spacing: 16px;

        // File selection section
        Text {
            text: "Select File";
            font-weight: 600;
            font-size: 14px;
        }

        HorizontalLayout {
            spacing: 8px;

            Rectangle {
                border-width: 1px;
                border-color: #ccc;
                border-radius: 4px;
                background: #fafafa;
                horizontal-stretch: 1;

                Text {
                    text: root.selected-file != "" ? root.selected-file : "No file selected";
                    color: root.selected-file != "" ? #333 : #999;
                    padding: 8px;
                }
            }

            Button {
                text: "Browse...";
                clicked => { root.browse-file(); }
            }
        }

        // Format selector
        HorizontalLayout {
            spacing: 8px;
            alignment: start;

            Text {
                text: "Format:";
                vertical-alignment: center;
            }

            ComboBox {
                model: ["Postman Collection v2.1", "Postman Environment"];
                current-index: root.selected-format == ImportFormat.PostmanV21 ? 0 : 1;
                selected(index) => {
                    root.format-changed(index == 0 ? ImportFormat.PostmanV21 : ImportFormat.PostmanEnvironment);
                }
            }
        }

        // Preview section (visible after validation)
        if root.state == ImportState.Previewing || root.state == ImportState.Importing || root.state == ImportState.Complete : Rectangle {
            border-width: 1px;
            border-color: #ddd;
            border-radius: 4px;
            background: #f9f9f9;
            padding: 12px;

            VerticalLayout {
                spacing: 8px;

                Text {
                    text: "Import Preview";
                    font-weight: 600;
                }

                if root.preview.collection-name != "" : Text {
                    text: "Collection: " + root.preview.collection-name;
                }

                if root.preview.environment-name != "" : Text {
                    text: "Environment: " + root.preview.environment-name;
                }

                HorizontalLayout {
                    spacing: 16px;

                    Text { text: "Requests: " + root.preview.request-count; }
                    Text { text: "Folders: " + root.preview.folder-count; }
                    Text { text: "Variables: " + root.preview.variable-count; }
                }
            }
        }

        // Warnings list (scrollable)
        if root.warnings.length > 0 : Rectangle {
            border-width: 1px;
            border-color: #f0ad4e;
            border-radius: 4px;
            background: #fcf8e3;
            min-height: 100px;
            max-height: 150px;

            VerticalLayout {
                padding: 8px;

                Text {
                    text: "Warnings (" + root.warnings.length + ")";
                    font-weight: 600;
                    color: #8a6d3b;
                }

                ListView {
                    for warning in root.warnings : Rectangle {
                        height: 24px;

                        HorizontalLayout {
                            spacing: 8px;

                            Rectangle {
                                width: 8px;
                                height: 8px;
                                border-radius: 4px;
                                background: warning.severity == "error" ? #d9534f :
                                           warning.severity == "warning" ? #f0ad4e : #5bc0de;
                            }

                            Text {
                                text: warning.path + ": " + warning.message;
                                font-size: 12px;
                                color: #666;
                                overflow: elide;
                            }
                        }
                    }
                }
            }
        }

        // Progress indicator
        if root.state == ImportState.Importing : VerticalLayout {
            spacing: 4px;

            Text {
                text: "Importing...";
                horizontal-alignment: center;
            }

            ProgressIndicator {
                progress: root.progress;
            }
        }

        // Error message
        if root.state == ImportState.Error : Rectangle {
            border-width: 1px;
            border-color: #d9534f;
            border-radius: 4px;
            background: #f2dede;
            padding: 12px;

            Text {
                text: root.error-message;
                color: #a94442;
                wrap: word-wrap;
            }
        }

        // Success message
        if root.state == ImportState.Complete : Rectangle {
            border-width: 1px;
            border-color: #5cb85c;
            border-radius: 4px;
            background: #dff0d8;
            padding: 12px;

            Text {
                text: "Import completed successfully!";
                color: #3c763d;
            }
        }

        // Spacer
        Rectangle { vertical-stretch: 1; }

        // Action buttons
        HorizontalLayout {
            spacing: 8px;
            alignment: end;

            Button {
                text: "Cancel";
                clicked => { root.cancel-import(); }
            }

            Button {
                text: root.state == ImportState.Complete ? "Close" : "Import";
                enabled: root.selected-file != "" &&
                         root.state != ImportState.Importing &&
                         root.state != ImportState.Validating;
                clicked => {
                    if root.state == ImportState.Complete {
                        root.cancel-import();
                    } else {
                        root.start-import();
                    }
                }
            }
        }
    }
}
```

### Rust UI State Handler

```rust
// ui/src/handlers/import_handler.rs

use slint::{ComponentHandle, Weak};
use crate::ImportDialog;
use application::use_cases::import_postman::{
    ImportPostmanCollection, ImportConfig, PostmanImporter,
};
use std::path::PathBuf;

pub struct ImportHandler {
    dialog: Weak<ImportDialog>,
    importer: PostmanImporter,
}

impl ImportHandler {
    pub fn new(dialog: Weak<ImportDialog>) -> Self {
        Self {
            dialog,
            importer: PostmanImporter::new(),
        }
    }

    /// Opens native file picker and updates UI.
    pub fn browse_file(&self) {
        let dialog = self.dialog.clone();

        // Use rfd (Rust File Dialog) for native file picker
        tokio::spawn(async move {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("Postman", &["json", "postman_collection.json"])
                .add_filter("All Files", &["*"])
                .pick_file()
                .await;

            if let Some(file) = file {
                let path = file.path().to_string_lossy().to_string();

                slint::invoke_from_event_loop(move || {
                    if let Some(dialog) = dialog.upgrade() {
                        dialog.set_selected_file(path.into());
                        // Trigger validation
                        dialog.set_state(ImportState::Validating);
                    }
                }).ok();
            }
        });
    }

    /// Validates and previews the selected file.
    pub async fn preview(&self, path: PathBuf) -> Result<(), String> {
        let dialog = self.dialog.clone();
        let config = ImportConfig::default();

        // Validate
        let validation = self.importer.validate_file(&path, &config).await
            .map_err(|e| e.message)?;

        if !validation.is_valid {
            return Err(validation.issues.join(", "));
        }

        // Preview
        let preview = self.importer.preview_import(&path, &config).await
            .map_err(|e| e.message)?;

        slint::invoke_from_event_loop(move || {
            if let Some(dialog) = dialog.upgrade() {
                dialog.set_state(ImportState::Previewing);
                dialog.set_preview(ImportPreviewData {
                    collection_name: preview.collection_name.unwrap_or_default().into(),
                    environment_name: preview.environment_name.unwrap_or_default().into(),
                    request_count: preview.request_count as i32,
                    folder_count: preview.folder_count as i32,
                    variable_count: preview.variable_count as i32,
                });

                // Set warnings
                let warnings: Vec<ImportWarningItem> = preview.warnings
                    .iter()
                    .map(|w| ImportWarningItem {
                        path: w.path.clone().into(),
                        message: w.message.clone().into(),
                        severity: match w.severity {
                            WarningSeverity::Info => "info",
                            WarningSeverity::Warning => "warning",
                            WarningSeverity::Error => "error",
                        }.into(),
                    })
                    .collect();
                dialog.set_warnings(warnings.into());
            }
        }).ok();

        Ok(())
    }

    /// Performs the actual import.
    pub async fn import(&self, path: PathBuf) -> Result<(), String> {
        let dialog = self.dialog.clone();
        let config = ImportConfig::default();

        // Update state
        slint::invoke_from_event_loop({
            let dialog = dialog.clone();
            move || {
                if let Some(dialog) = dialog.upgrade() {
                    dialog.set_state(ImportState::Importing);
                    dialog.set_progress(0.0);
                }
            }
        }).ok();

        // Perform import
        let result = self.importer.import_collection(&path, &config).await
            .map_err(|e| e.message)?;

        // Update progress (simulate for now)
        slint::invoke_from_event_loop({
            let dialog = dialog.clone();
            move || {
                if let Some(dialog) = dialog.upgrade() {
                    dialog.set_progress(1.0);
                    dialog.set_state(ImportState::Complete);

                    // Update warnings with any new ones from import
                    let warnings: Vec<ImportWarningItem> = result.warnings
                        .iter()
                        .map(|w| ImportWarningItem {
                            path: w.path.clone().into(),
                            message: w.message.clone().into(),
                            severity: match w.severity {
                                WarningSeverity::Info => "info",
                                WarningSeverity::Warning => "warning",
                                WarningSeverity::Error => "error",
                            }.into(),
                        })
                        .collect();
                    dialog.set_warnings(warnings.into());
                }
            }
        }).ok();

        // TODO: Persist the imported collection using the repository

        Ok(())
    }
}
```

---

## Testing Strategy

Place tests in: `infrastructure/tests/postman_import/`

### Sample Postman Collection for Tests

```rust
// infrastructure/tests/fixtures/sample_collection.rs

pub const MINIMAL_COLLECTION: &str = r#"{
    "info": {
        "_postman_id": "12345678-1234-1234-1234-123456789012",
        "name": "Minimal Test Collection",
        "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
    },
    "item": [
        {
            "name": "Simple GET",
            "request": {
                "method": "GET",
                "url": "https://api.example.com/users"
            }
        }
    ]
}"#;

pub const FULL_COLLECTION: &str = r#"{
    "info": {
        "_postman_id": "12345678-1234-1234-1234-123456789012",
        "name": "Full Test Collection",
        "description": "A comprehensive test collection",
        "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
    },
    "item": [
        {
            "name": "Auth Folder",
            "item": [
                {
                    "name": "Login",
                    "request": {
                        "method": "POST",
                        "header": [
                            {
                                "key": "Content-Type",
                                "value": "application/json"
                            }
                        ],
                        "body": {
                            "mode": "raw",
                            "raw": "{\"username\": \"{{user}}\", \"password\": \"{{pass}}\"}",
                            "options": {
                                "raw": {
                                    "language": "json"
                                }
                            }
                        },
                        "url": {
                            "raw": "{{base_url}}/auth/login",
                            "host": ["{{base_url}}"],
                            "path": ["auth", "login"]
                        },
                        "auth": {
                            "type": "noauth"
                        }
                    }
                },
                {
                    "name": "Logout",
                    "request": {
                        "method": "POST",
                        "url": "{{base_url}}/auth/logout",
                        "auth": {
                            "type": "bearer",
                            "bearer": [
                                {
                                    "key": "token",
                                    "value": "{{access_token}}",
                                    "type": "string"
                                }
                            ]
                        }
                    }
                }
            ]
        },
        {
            "name": "Get Users",
            "request": {
                "method": "GET",
                "header": [
                    {
                        "key": "Accept",
                        "value": "application/json"
                    },
                    {
                        "key": "X-Disabled",
                        "value": "should-not-appear",
                        "disabled": true
                    }
                ],
                "url": {
                    "raw": "{{base_url}}/users?page=1&limit=10",
                    "host": ["{{base_url}}"],
                    "path": ["users"],
                    "query": [
                        {
                            "key": "page",
                            "value": "1"
                        },
                        {
                            "key": "limit",
                            "value": "10"
                        }
                    ]
                },
                "auth": {
                    "type": "apikey",
                    "apikey": [
                        {
                            "key": "key",
                            "value": "X-API-Key",
                            "type": "string"
                        },
                        {
                            "key": "value",
                            "value": "{{api_key}}",
                            "type": "string"
                        },
                        {
                            "key": "in",
                            "value": "header",
                            "type": "string"
                        }
                    ]
                }
            }
        },
        {
            "name": "Create User",
            "request": {
                "method": "POST",
                "body": {
                    "mode": "urlencoded",
                    "urlencoded": [
                        {
                            "key": "name",
                            "value": "John Doe",
                            "type": "text"
                        },
                        {
                            "key": "email",
                            "value": "john@example.com",
                            "type": "text"
                        }
                    ]
                },
                "url": "{{base_url}}/users"
            }
        },
        {
            "name": "Upload Avatar",
            "request": {
                "method": "POST",
                "body": {
                    "mode": "formdata",
                    "formdata": [
                        {
                            "key": "avatar",
                            "type": "file",
                            "src": "/path/to/avatar.png"
                        },
                        {
                            "key": "description",
                            "value": "Profile picture",
                            "type": "text"
                        }
                    ]
                },
                "url": "{{base_url}}/users/{{user_id}}/avatar"
            }
        }
    ],
    "variable": [
        {
            "key": "base_url",
            "value": "https://api.example.com",
            "type": "string"
        },
        {
            "key": "api_key",
            "value": "",
            "type": "string"
        }
    ],
    "event": [
        {
            "listen": "prerequest",
            "script": {
                "type": "text/javascript",
                "exec": ["console.log('Pre-request script');"]
            }
        }
    ]
}"#;

pub const DEEPLY_NESTED_COLLECTION: &str = r#"{
    "info": {
        "_postman_id": "deep-1234",
        "name": "Deeply Nested",
        "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
    },
    "item": [
        {
            "name": "Level 1",
            "item": [
                {
                    "name": "Level 2",
                    "item": [
                        {
                            "name": "Level 3",
                            "item": [
                                {
                                    "name": "Level 4",
                                    "item": [
                                        {
                                            "name": "Level 5",
                                            "item": [
                                                {
                                                    "name": "Deep Request",
                                                    "request": {
                                                        "method": "GET",
                                                        "url": "https://api.example.com/deep"
                                                    }
                                                }
                                            ]
                                        }
                                    ]
                                }
                            ]
                        }
                    ]
                }
            ]
        }
    ]
}"#;

pub const UNSUPPORTED_AUTH_COLLECTION: &str = r#"{
    "info": {
        "_postman_id": "unsupported-auth",
        "name": "Unsupported Auth Types",
        "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
    },
    "item": [
        {
            "name": "AWS Signed Request",
            "request": {
                "method": "GET",
                "url": "https://s3.amazonaws.com/bucket/object",
                "auth": {
                    "type": "awsv4",
                    "awsv4": [
                        {"key": "accessKey", "value": "AKIA...", "type": "string"},
                        {"key": "secretKey", "value": "secret", "type": "string"},
                        {"key": "region", "value": "us-east-1", "type": "string"},
                        {"key": "service", "value": "s3", "type": "string"}
                    ]
                }
            }
        }
    ]
}"#;

pub const SAMPLE_ENVIRONMENT: &str = r#"{
    "id": "env-12345678",
    "name": "Development",
    "values": [
        {
            "key": "base_url",
            "value": "http://localhost:3000",
            "enabled": true,
            "type": "default"
        },
        {
            "key": "api_key",
            "value": "dev-secret-key",
            "enabled": true,
            "type": "secret"
        },
        {
            "key": "disabled_var",
            "value": "should-not-import",
            "enabled": false
        }
    ],
    "_postman_variable_scope": "environment"
}"#;
```

### Unit Tests

```rust
// infrastructure/tests/postman_import/parsing_tests.rs

use infrastructure::import::postman::types::*;

mod fixtures;
use fixtures::*;

#[test]
fn test_parse_minimal_collection() {
    let collection: PostmanCollection = serde_json::from_str(MINIMAL_COLLECTION)
        .expect("Failed to parse minimal collection");

    assert_eq!(collection.info.name, "Minimal Test Collection");
    assert_eq!(collection.item.len(), 1);
    assert!(collection.item[0].is_request());
}

#[test]
fn test_parse_full_collection() {
    let collection: PostmanCollection = serde_json::from_str(FULL_COLLECTION)
        .expect("Failed to parse full collection");

    assert_eq!(collection.info.name, "Full Test Collection");
    assert_eq!(collection.variable.len(), 2);
    assert!(!collection.event.is_empty()); // Has scripts

    // Check folder structure
    let auth_folder = &collection.item[0];
    assert!(auth_folder.is_folder());
    assert_eq!(auth_folder.item.len(), 2);
}

#[test]
fn test_parse_structured_url() {
    let collection: PostmanCollection = serde_json::from_str(FULL_COLLECTION)
        .expect("Failed to parse collection");

    // Get Users request has structured URL
    let get_users = &collection.item[1];
    let request = get_users.request.as_ref().unwrap();

    let url_string = request.url.to_url_string();
    assert!(url_string.contains("{{base_url}}"));

    let query_params = request.url.query_params();
    assert_eq!(query_params.len(), 2);
}

#[test]
fn test_parse_auth_types() {
    let collection: PostmanCollection = serde_json::from_str(FULL_COLLECTION)
        .expect("Failed to parse collection");

    // Bearer auth
    let logout = &collection.item[0].item[1];
    let auth = logout.request.as_ref().unwrap().auth.as_ref().unwrap();
    assert_eq!(auth.auth_type, "bearer");
    assert_eq!(auth.get_param("token"), Some("{{access_token}}".to_string()));

    // API Key auth
    let get_users = &collection.item[1];
    let auth = get_users.request.as_ref().unwrap().auth.as_ref().unwrap();
    assert_eq!(auth.auth_type, "apikey");
    assert_eq!(auth.get_param("key"), Some("X-API-Key".to_string()));
}

#[test]
fn test_parse_body_modes() {
    let collection: PostmanCollection = serde_json::from_str(FULL_COLLECTION)
        .expect("Failed to parse collection");

    // Raw JSON body
    let login = &collection.item[0].item[0];
    let body = login.request.as_ref().unwrap().body.as_ref().unwrap();
    assert_eq!(body.mode, "raw");

    // URL encoded body
    let create_user = &collection.item[2];
    let body = create_user.request.as_ref().unwrap().body.as_ref().unwrap();
    assert_eq!(body.mode, "urlencoded");
    assert_eq!(body.urlencoded.len(), 2);

    // Form data body
    let upload = &collection.item[3];
    let body = upload.request.as_ref().unwrap().body.as_ref().unwrap();
    assert_eq!(body.mode, "formdata");
    assert_eq!(body.formdata.len(), 2);
}

#[test]
fn test_disabled_headers_filtered() {
    let collection: PostmanCollection = serde_json::from_str(FULL_COLLECTION)
        .expect("Failed to parse collection");

    let get_users = &collection.item[1];
    let headers = &get_users.request.as_ref().unwrap().header;

    // Should have 2 headers, one disabled
    assert_eq!(headers.len(), 2);

    let enabled_headers: Vec<_> = headers
        .iter()
        .filter(|h| h.disabled != Some(true))
        .collect();
    assert_eq!(enabled_headers.len(), 1);
}

#[test]
fn test_parse_environment() {
    let env: PostmanEnvironment = serde_json::from_str(SAMPLE_ENVIRONMENT)
        .expect("Failed to parse environment");

    assert_eq!(env.name, "Development");
    assert_eq!(env.values.len(), 3);

    // Check secret type
    let secret_var = env.values.iter().find(|v| v.key == "api_key").unwrap();
    assert_eq!(secret_var.var_type, Some("secret".to_string()));
}
```

### Integration Tests

```rust
// infrastructure/tests/postman_import/import_tests.rs

use application::use_cases::import_postman::*;
use std::io::Write;
use tempfile::NamedTempFile;

mod fixtures;
use fixtures::*;

#[tokio::test]
async fn test_import_minimal_collection() {
    let importer = PostmanImporter::new();
    let config = ImportConfig::default();

    // Write test data to temp file
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(MINIMAL_COLLECTION.as_bytes()).unwrap();

    let result = importer.import_collection(file.path(), &config).await.unwrap();

    assert!(result.collection.is_some());
    let collection = result.collection.unwrap();
    assert_eq!(collection.name, "Minimal Test Collection");
    assert_eq!(result.stats.requests_imported, 1);
    assert!(result.warnings.is_empty());
}

#[tokio::test]
async fn test_import_with_scripts_generates_warnings() {
    let importer = PostmanImporter::new();
    let config = ImportConfig::default();

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(FULL_COLLECTION.as_bytes()).unwrap();

    let result = importer.import_collection(file.path(), &config).await.unwrap();

    // Should have warnings about scripts
    assert!(!result.warnings.is_empty());
    assert!(result.warnings.iter().any(|w| w.message.contains("script")));
}

#[tokio::test]
async fn test_import_respects_max_depth() {
    let importer = PostmanImporter::new();
    let config = ImportConfig {
        max_depth: 3,
        ..Default::default()
    };

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(DEEPLY_NESTED_COLLECTION.as_bytes()).unwrap();

    let result = importer.import_collection(file.path(), &config).await.unwrap();

    // Should warn about exceeding depth
    assert!(result.warnings.iter().any(|w| w.message.contains("depth")));
}

#[tokio::test]
async fn test_import_unsupported_auth_generates_warning() {
    let importer = PostmanImporter::new();
    let config = ImportConfig::default();

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(UNSUPPORTED_AUTH_COLLECTION.as_bytes()).unwrap();

    let result = importer.import_collection(file.path(), &config).await.unwrap();

    // Should have warning about AWS auth
    assert!(result.warnings.iter().any(|w| w.message.contains("awsv4")));

    // Request should still be imported, just without auth
    assert_eq!(result.stats.requests_imported, 1);
}

#[tokio::test]
async fn test_import_environment() {
    let importer = PostmanImporter::new();
    let config = ImportConfig::default();

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(SAMPLE_ENVIRONMENT.as_bytes()).unwrap();

    let result = importer.import_environment(file.path(), &config).await.unwrap();

    assert_eq!(result.environments.len(), 1);
    let env = &result.environments[0];
    assert_eq!(env.name, "Development");

    // Disabled variable should not be imported
    assert_eq!(env.variables.len(), 2);
    assert!(!env.variables.contains_key("disabled_var"));

    // Secret variable should be marked as secret
    let api_key = env.variables.get("api_key").unwrap();
    assert!(api_key.secret);
}

#[tokio::test]
async fn test_validate_file_size_limit() {
    let importer = PostmanImporter::new();
    let config = ImportConfig {
        max_file_size: 100, // Very small limit
        ..Default::default()
    };

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(FULL_COLLECTION.as_bytes()).unwrap();

    let validation = importer.validate_file(file.path(), &config).await.unwrap();

    assert!(!validation.is_valid);
    assert!(validation.issues.iter().any(|i| i.contains("size")));
}

#[tokio::test]
async fn test_preview_import() {
    let importer = PostmanImporter::new();
    let config = ImportConfig::default();

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(FULL_COLLECTION.as_bytes()).unwrap();

    let preview = importer.preview_import(file.path(), &config).await.unwrap();

    assert_eq!(preview.collection_name, Some("Full Test Collection".to_string()));
    assert_eq!(preview.detected_format, DetectedFormat::PostmanCollectionV21);
    assert!(preview.request_count > 0);
    assert!(preview.folder_count > 0);
}

#[tokio::test]
async fn test_format_detection() {
    let importer = PostmanImporter::new();
    let config = ImportConfig::default();

    // Collection
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(MINIMAL_COLLECTION.as_bytes()).unwrap();
    let validation = importer.validate_file(file.path(), &config).await.unwrap();
    assert_eq!(validation.detected_format, DetectedFormat::PostmanCollectionV21);

    // Environment
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(SAMPLE_ENVIRONMENT.as_bytes()).unwrap();
    let validation = importer.validate_file(file.path(), &config).await.unwrap();
    assert_eq!(validation.detected_format, DetectedFormat::PostmanEnvironment);
}
```

---

## Implementation Order

Follow this sequence for implementation:

### Phase 1: Data Types (Day 1-2)

1. **Create module structure**
   - `infrastructure/src/import/mod.rs`
   - `infrastructure/src/import/postman/mod.rs`
   - `infrastructure/src/import/postman/types.rs`
   - `infrastructure/src/import/postman/environment_types.rs`

2. **Implement Postman struct definitions**
   - Start with `PostmanCollection`, `PostmanInfo`, `PostmanItem`
   - Add `PostmanRequest`, `PostmanUrl`, `PostmanUrlStructured`
   - Add `PostmanHeader`, `PostmanBody`, `PostmanAuth`
   - Add `PostmanVariable`, `PostmanEvent`
   - Write unit tests for JSON parsing

3. **Add test fixtures**
   - Create sample collection JSON files
   - Verify all structs parse correctly

### Phase 2: Mapping Logic (Day 3-4)

4. **Create mapper module**
   - `infrastructure/src/import/postman/mapper.rs`

5. **Implement mapping functions** (in order)
   - `map_http_method` - simplest
   - `map_headers` - straightforward
   - `map_query_params` and URL reconstruction
   - `map_body` - handle all body modes
   - `map_auth` - handle supported auth types
   - `map_postman_item` - recursive, folders and requests
   - `map_collection_variables`
   - `map_postman_environment`

6. **Add warning infrastructure**
   - `ImportWarning` struct
   - Collect warnings during mapping

### Phase 3: Use Case Implementation (Day 5-6)

7. **Create use case trait and implementation**
   - `application/src/use_cases/import_postman.rs`
   - `ImportPostmanCollection` trait
   - `PostmanImporter` implementation

8. **Implement use case methods** (in order)
   - `validate_file` - file existence, size, format detection
   - `preview_import` - parse and count without persisting
   - `import_collection` - full import flow
   - `import_environment` - environment import

9. **Add validation logic**
   - Max file size check
   - Max depth enforcement
   - Max items limit
   - JSON validity check

### Phase 4: UI Components (Day 7-8)

10. **Create Slint UI components**
    - `ui/src/components/import/import_dialog.slint`
    - File picker integration
    - Preview display
    - Warnings list
    - Progress indicator

11. **Create Rust UI handler**
    - `ui/src/handlers/import_handler.rs`
    - Wire up callbacks
    - Handle async operations
    - Update UI state

12. **Integrate with main application**
    - Add menu item / button for import
    - Connect dialog to main window

### Phase 5: Testing and Polish (Day 9-10)

13. **Integration testing**
    - Test with real Postman exports
    - Test edge cases (large files, deeply nested)
    - Test error handling

14. **Documentation**
    - User-facing help text
    - Code documentation
    - Update README if needed

15. **Final polish**
    - Error message clarity
    - Warning message helpfulness
    - UI polish and feedback

---

## Acceptance Criteria

### Functional Requirements

- [ ] Can import a Postman Collection v2.1 JSON file
- [ ] Can import a Postman Environment JSON file
- [ ] All HTTP methods are correctly mapped
- [ ] Headers are correctly mapped (disabled headers excluded)
- [ ] URL with query parameters is correctly reconstructed
- [ ] Body types are correctly mapped: raw, urlencoded, formdata, graphql
- [ ] Auth types are correctly mapped: noauth, basic, bearer, apikey, oauth2 (partial)
- [ ] Collection variables are imported
- [ ] Nested folders are preserved (up to max depth)
- [ ] Unsupported features generate clear warnings
- [ ] Invalid JSON is rejected with helpful error message
- [ ] Files exceeding size limit are rejected
- [ ] Import can be previewed before execution
- [ ] Progress is shown during import

### Non-Functional Requirements

- [ ] Import of 100-request collection completes in < 2 seconds
- [ ] UI remains responsive during import (async operation)
- [ ] Memory usage stays reasonable for large files (streaming if needed)
- [ ] All warning messages are actionable and clear
- [ ] No panics on malformed input

### Testing Requirements

- [ ] Unit tests for all Postman struct parsing
- [ ] Unit tests for all mapping functions
- [ ] Integration tests for full import flow
- [ ] Test coverage for edge cases (empty collections, deeply nested, special characters)
- [ ] Test with real-world Postman exports

---

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Postman format variations | High | Medium | Use `#[serde(default)]` extensively, test with many real exports |
| Unsupported auth types | Medium | Low | Clear warnings, document limitations, auth is optional |
| Large file performance | Medium | Medium | Set reasonable limits, consider streaming for very large files |
| Script dependencies in requests | High | Low | Document that scripts are skipped, suggest manual conversion |
| Variable syntax differences | Low | Medium | Map `{{var}}` syntax directly (compatible) |
| Character encoding issues | Low | Low | Enforce UTF-8, reject other encodings with clear error |

---

## Dependencies

### Rust Crates

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4", "serde"] }
async-trait = "0.1"
tokio = { version = "1.0", features = ["fs", "macros"] }
rfd = "0.14"  # Native file dialogs
thiserror = "1.0"

[dev-dependencies]
tempfile = "3.0"
```

### Domain Types Required

From `domain` crate (should already exist from previous sprints):
- `Request`, `RequestId`
- `Folder`, `FolderId`
- `Collection`, `CollectionId`
- `Environment`, `EnvironmentId`
- `HttpMethod`
- `Auth` (with variants: Bearer, Basic, ApiKey, OAuth2ClientCredentials, OAuth2AuthCode)
- `Body` (with variants: Json, Text, FormUrlEncoded, FormData, Binary, GraphQL)
- `FormField`, `FormFieldType`
- `Variable`, `VariableValue`

---

## Related Documents

- `02-file-format-spec.md` - Vortex native format specification
- `01-competitive-analysis.md` - Postman format analysis
- Sprint 01-03 documentation for existing domain types
