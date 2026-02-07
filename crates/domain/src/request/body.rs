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
    #[allow(clippy::missing_const_for_fn)] // String::is_empty is not const
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
