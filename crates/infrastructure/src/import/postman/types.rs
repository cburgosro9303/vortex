//! Postman Collection v2.1 Type Definitions
//!
//! This module defines the types that represent a Postman Collection v2.1 JSON file.
//! All types use `#[serde(default)]` extensively to handle format variations gracefully.

#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

/// Root structure for Postman Collection v2.1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanCollection {
    pub info: PostmanInfo,
    #[serde(default)]
    pub item: Vec<PostmanItem>,
    #[serde(default)]
    pub variable: Vec<PostmanVariable>,
    #[serde(default)]
    pub auth: Option<PostmanAuth>,
    #[serde(default)]
    pub event: Vec<PostmanEvent>,
}

/// Collection metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanInfo {
    pub name: String,
    #[serde(rename = "_postman_id", default)]
    pub postman_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(rename = "_exporter_id", default)]
    pub exporter_id: Option<String>,
}

/// An item can be either a folder (containing more items) or a request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanItem {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// If present, this item is a folder containing sub-items
    #[serde(default)]
    pub item: Option<Vec<Self>>,
    /// If present, this item is a request
    #[serde(default)]
    pub request: Option<PostmanRequest>,
    /// Response examples
    #[serde(default)]
    pub response: Vec<serde_json::Value>,
    /// Events (scripts) attached to this item
    #[serde(default)]
    pub event: Vec<PostmanEvent>,
    /// Item-level auth override
    #[serde(default)]
    pub auth: Option<PostmanAuth>,
}

impl PostmanItem {
    /// Returns true if this item is a folder (has sub-items)
    #[must_use]
    pub const fn is_folder(&self) -> bool {
        self.item.is_some()
    }

    /// Returns true if this item is a request
    #[must_use]
    pub const fn is_request(&self) -> bool {
        self.request.is_some()
    }
}

/// Postman Request definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanRequest {
    pub method: String,
    #[serde(default)]
    pub url: PostmanUrl,
    #[serde(default)]
    pub header: Vec<PostmanHeader>,
    #[serde(default)]
    pub body: Option<PostmanBody>,
    #[serde(default)]
    pub auth: Option<PostmanAuth>,
    #[serde(default)]
    pub description: Option<String>,
}

/// URL can be either a simple string or a structured object
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum PostmanUrl {
    #[default]
    Empty,
    Simple(String),
    Structured(PostmanUrlStructured),
}

impl PostmanUrl {
    /// Get the raw URL string
    #[must_use]
    pub fn raw(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Simple(s) => s.clone(),
            Self::Structured(s) => s.raw.clone().unwrap_or_default(),
        }
    }

    /// Get query parameters if available
    #[must_use]
    pub fn query_params(&self) -> Vec<PostmanQueryParam> {
        match self {
            Self::Structured(s) => s.query.clone(),
            _ => Vec::new(),
        }
    }
}

/// Structured URL object
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PostmanUrlStructured {
    #[serde(default)]
    pub raw: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub host: Vec<String>,
    #[serde(default)]
    pub port: Option<String>,
    #[serde(default)]
    pub path: Vec<String>,
    #[serde(default)]
    pub query: Vec<PostmanQueryParam>,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub variable: Vec<PostmanPathVariable>,
}

/// Query parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanQueryParam {
    pub key: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

/// Path variable (for URL templates like :id)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanPathVariable {
    pub key: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Request header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanHeader {
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "type")]
    pub header_type: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

/// Request body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanBody {
    pub mode: String,
    #[serde(default)]
    pub raw: Option<String>,
    #[serde(default)]
    pub urlencoded: Vec<PostmanFormParam>,
    #[serde(default)]
    pub formdata: Vec<PostmanFormDataParam>,
    #[serde(default)]
    pub file: Option<PostmanBodyFile>,
    #[serde(default)]
    pub graphql: Option<PostmanGraphQL>,
    #[serde(default)]
    pub options: Option<PostmanBodyOptions>,
}

/// Form URL-encoded parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanFormParam {
    pub key: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type", default)]
    pub param_type: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

/// Form-data parameter (supports file uploads)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanFormDataParam {
    pub key: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub src: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type", default)]
    pub param_type: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(rename = "contentType", default)]
    pub content_type: Option<String>,
}

/// Binary file body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanBodyFile {
    #[serde(default)]
    pub src: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

/// GraphQL body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanGraphQL {
    pub query: String,
    #[serde(default)]
    pub variables: Option<String>,
    #[serde(rename = "operationName", default)]
    pub operation_name: Option<String>,
}

/// Body options (e.g., raw language)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanBodyOptions {
    #[serde(default)]
    pub raw: Option<PostmanRawOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanRawOptions {
    #[serde(default)]
    pub language: Option<String>,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanAuth {
    #[serde(rename = "type")]
    pub auth_type: String,
    #[serde(default)]
    pub noauth: Option<serde_json::Value>,
    #[serde(default)]
    pub basic: Vec<PostmanAuthParam>,
    #[serde(default)]
    pub bearer: Vec<PostmanAuthParam>,
    #[serde(default)]
    pub apikey: Vec<PostmanAuthParam>,
    #[serde(default)]
    pub oauth2: Vec<PostmanAuthParam>,
    #[serde(default)]
    pub digest: Vec<PostmanAuthParam>,
    #[serde(default)]
    pub hawk: Vec<PostmanAuthParam>,
    #[serde(default)]
    pub ntlm: Vec<PostmanAuthParam>,
    #[serde(default)]
    pub awsv4: Vec<PostmanAuthParam>,
}

impl PostmanAuth {
    /// Get a parameter value by key
    #[must_use]
    pub fn get_param(&self, params: &[PostmanAuthParam], key: &str) -> Option<String> {
        params
            .iter()
            .find(|p| p.key == key)
            .and_then(|p| p.value.clone())
    }
}

/// Auth parameter (key-value pair)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanAuthParam {
    pub key: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(rename = "type", default)]
    pub param_type: Option<String>,
}

/// Variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanVariable {
    pub key: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(rename = "type", default)]
    pub var_type: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

/// Event (pre-request or test script)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanEvent {
    pub listen: String,
    #[serde(default)]
    pub script: Option<PostmanScript>,
}

/// Script definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanScript {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type", default)]
    pub script_type: Option<String>,
    #[serde(default)]
    pub exec: Vec<String>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_collection() {
        let json = r#"{
            "info": {
                "name": "Test Collection",
                "_postman_id": "abc123",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": []
        }"#;

        let collection: PostmanCollection = serde_json::from_str(json).unwrap();
        assert_eq!(collection.info.name, "Test Collection");
        assert!(collection.item.is_empty());
    }

    #[test]
    fn test_parse_request_with_body() {
        let json = r#"{
            "info": {"name": "Test"},
            "item": [{
                "name": "Create User",
                "request": {
                    "method": "POST",
                    "url": "https://api.example.com/users",
                    "header": [
                        {"key": "Content-Type", "value": "application/json"}
                    ],
                    "body": {
                        "mode": "raw",
                        "raw": "{\"name\": \"John\"}"
                    }
                }
            }]
        }"#;

        let collection: PostmanCollection = serde_json::from_str(json).unwrap();
        assert_eq!(collection.item.len(), 1);
        let item = &collection.item[0];
        assert!(item.is_request());
        let request = item.request.as_ref().unwrap();
        assert_eq!(request.method, "POST");
    }

    #[test]
    fn test_parse_structured_url() {
        let json = r#"{
            "raw": "https://api.example.com/users?page=1",
            "protocol": "https",
            "host": ["api", "example", "com"],
            "path": ["users"],
            "query": [{"key": "page", "value": "1"}]
        }"#;

        let url: PostmanUrlStructured = serde_json::from_str(json).unwrap();
        assert_eq!(
            url.raw,
            Some("https://api.example.com/users?page=1".to_string())
        );
        assert_eq!(url.query.len(), 1);
    }

    #[test]
    fn test_parse_auth_types() {
        let json = r#"{
            "type": "bearer",
            "bearer": [{"key": "token", "value": "abc123"}]
        }"#;

        let auth: PostmanAuth = serde_json::from_str(json).unwrap();
        assert_eq!(auth.auth_type, "bearer");
        assert_eq!(
            auth.get_param(&auth.bearer, "token"),
            Some("abc123".to_string())
        );
    }
}
