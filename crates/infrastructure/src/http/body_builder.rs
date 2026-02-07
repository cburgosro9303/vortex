//! HTTP request body builder.
//!
//! This module provides utilities for building various HTTP request body types
//! from the domain `PersistenceRequestBody` type.

#![allow(missing_docs)]

use reqwest::multipart::{Form, Part};
use std::path::Path;
use vortex_domain::persistence::{FormDataField, PersistenceRequestBody};

/// Error type for body building operations.
#[derive(Debug, thiserror::Error)]
pub enum BodyBuildError {
    /// File not found.
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    /// Failed to read file.
    #[error("Failed to read file: {message}")]
    FileReadError { message: String },

    /// Invalid body configuration.
    #[error("Invalid body configuration: {message}")]
    InvalidConfig { message: String },

    /// Serialization error.
    #[error("Serialization error: {message}")]
    SerializationError { message: String },
}

/// Result of building a body.
pub enum BuiltBody {
    /// No body.
    None,
    /// Text/JSON body with content type.
    Text {
        content: String,
        content_type: String,
    },
    /// Binary body from file.
    Binary {
        content: Vec<u8>,
        content_type: String,
    },
    /// Multipart form data.
    Multipart(Form),
}

/// Build an HTTP body from a persistence body type.
#[allow(clippy::missing_errors_doc)]
pub async fn build_body(
    body: &PersistenceRequestBody,
    workspace_path: Option<&Path>,
) -> Result<BuiltBody, BodyBuildError> {
    match body {
        PersistenceRequestBody::Json { content } => {
            let json_str =
                serde_json::to_string(content).map_err(|e| BodyBuildError::SerializationError {
                    message: e.to_string(),
                })?;
            Ok(BuiltBody::Text {
                content: json_str,
                content_type: "application/json".to_string(),
            })
        }

        PersistenceRequestBody::Text { content } => Ok(BuiltBody::Text {
            content: content.clone(),
            content_type: "text/plain".to_string(),
        }),

        PersistenceRequestBody::FormUrlencoded { fields } => {
            let encoded = serde_urlencoded::to_string(fields).map_err(|e| {
                BodyBuildError::SerializationError {
                    message: e.to_string(),
                }
            })?;
            Ok(BuiltBody::Text {
                content: encoded,
                content_type: "application/x-www-form-urlencoded".to_string(),
            })
        }

        PersistenceRequestBody::FormData { fields } => {
            let form = build_multipart_form(fields, workspace_path).await?;
            Ok(BuiltBody::Multipart(form))
        }

        PersistenceRequestBody::Binary { path } => {
            let file_path = resolve_path(path, workspace_path);
            let content =
                tokio::fs::read(&file_path)
                    .await
                    .map_err(|e| BodyBuildError::FileReadError {
                        message: format!("{}: {}", file_path.display(), e),
                    })?;

            let content_type = mime_guess::from_path(&file_path)
                .first_or_octet_stream()
                .to_string();

            Ok(BuiltBody::Binary {
                content,
                content_type,
            })
        }

        PersistenceRequestBody::Graphql { query, variables } => {
            let graphql_body = serde_json::json!({
                "query": query,
                "variables": variables,
            });
            let json_str = serde_json::to_string(&graphql_body).map_err(|e| {
                BodyBuildError::SerializationError {
                    message: e.to_string(),
                }
            })?;
            Ok(BuiltBody::Text {
                content: json_str,
                content_type: "application/json".to_string(),
            })
        }
    }
}

/// Build a multipart form from form data fields.
async fn build_multipart_form(
    fields: &[FormDataField],
    workspace_path: Option<&Path>,
) -> Result<Form, BodyBuildError> {
    let mut form = Form::new();

    for field in fields {
        match field {
            FormDataField::Text { name, value } => {
                form = form.text(name.clone(), value.clone());
            }
            FormDataField::File { name, path } => {
                let file_path = resolve_path(path, workspace_path);

                // Read file content
                let content = tokio::fs::read(&file_path).await.map_err(|e| {
                    BodyBuildError::FileReadError {
                        message: format!("{}: {}", file_path.display(), e),
                    }
                })?;

                // Get filename
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();

                // Get mime type
                let mime_type = mime_guess::from_path(&file_path)
                    .first_or_octet_stream()
                    .to_string();

                let part = Part::bytes(content)
                    .file_name(filename)
                    .mime_str(&mime_type)
                    .map_err(|e| BodyBuildError::InvalidConfig {
                        message: format!("Invalid MIME type: {e}"),
                    })?;

                form = form.part(name.clone(), part);
            }
        }
    }

    Ok(form)
}

/// Resolve a path relative to workspace or as absolute.
fn resolve_path(path: &str, workspace_path: Option<&Path>) -> std::path::PathBuf {
    let path = std::path::Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(ws) = workspace_path {
        ws.join(path)
    } else {
        path.to_path_buf()
    }
}

/// Get the content type for a built body.
impl BuiltBody {
    /// Get the Content-Type header value.
    #[must_use] 
    pub fn content_type(&self) -> Option<&str> {
        match self {
            Self::Text { content_type, .. } | Self::Binary { content_type, .. } => Some(content_type),
            Self::None | Self::Multipart(_) => None, // reqwest sets this automatically with boundary
        }
    }

    /// Check if this is a multipart form.
    #[must_use] 
    pub const fn is_multipart(&self) -> bool {
        matches!(self, Self::Multipart(_))
    }

    /// Check if this body is empty/none.
    #[must_use] 
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn test_build_json_body() {
        let body = PersistenceRequestBody::Json {
            content: serde_json::json!({"key": "value"}),
        };

        let result = build_body(&body, None).await.unwrap();
        match result {
            BuiltBody::Text {
                content,
                content_type,
            } => {
                assert_eq!(content_type, "application/json");
                assert!(content.contains("key"));
            }
            _ => panic!("Expected Text body"),
        }
    }

    #[tokio::test]
    async fn test_build_text_body() {
        let body = PersistenceRequestBody::Text {
            content: "Hello, World!".to_string(),
        };

        let result = build_body(&body, None).await.unwrap();
        match result {
            BuiltBody::Text {
                content,
                content_type,
            } => {
                assert_eq!(content, "Hello, World!");
                assert_eq!(content_type, "text/plain");
            }
            _ => panic!("Expected Text body"),
        }
    }

    #[tokio::test]
    async fn test_build_form_urlencoded() {
        let mut fields = BTreeMap::new();
        fields.insert("username".to_string(), "john".to_string());
        fields.insert("password".to_string(), "secret".to_string());

        let body = PersistenceRequestBody::FormUrlencoded { fields };

        let result = build_body(&body, None).await.unwrap();
        match result {
            BuiltBody::Text {
                content,
                content_type,
            } => {
                assert_eq!(content_type, "application/x-www-form-urlencoded");
                assert!(content.contains("username=john"));
            }
            _ => panic!("Expected Text body"),
        }
    }

    #[tokio::test]
    async fn test_build_graphql_body() {
        let body = PersistenceRequestBody::Graphql {
            query: "query { user { id name } }".to_string(),
            variables: Some(serde_json::json!({"id": "123"})),
        };

        let result = build_body(&body, None).await.unwrap();
        match result {
            BuiltBody::Text {
                content,
                content_type,
            } => {
                assert_eq!(content_type, "application/json");
                assert!(content.contains("query"));
                assert!(content.contains("variables"));
            }
            _ => panic!("Expected Text body"),
        }
    }

    #[test]
    fn test_resolve_path_absolute() {
        let path = resolve_path("/absolute/path/file.txt", Some(Path::new("/workspace")));
        assert_eq!(path, std::path::PathBuf::from("/absolute/path/file.txt"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let path = resolve_path("relative/file.txt", Some(Path::new("/workspace")));
        assert_eq!(
            path,
            std::path::PathBuf::from("/workspace/relative/file.txt")
        );
    }
}
