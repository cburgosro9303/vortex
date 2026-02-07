//! Request body types for various content formats.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

/// Request body with multiple format support.
///
/// The `type` field is used as the discriminator for JSON serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PersistenceRequestBody {
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

impl PersistenceRequestBody {
    /// Creates a JSON body from a `serde_json::Value`.
    #[must_use]
    pub const fn json(content: JsonValue) -> Self {
        Self::Json { content }
    }

    /// Creates a text body.
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text {
            content: content.into(),
        }
    }

    /// Creates a form-urlencoded body from key-value pairs.
    #[must_use]
    pub const fn form_urlencoded(fields: BTreeMap<String, String>) -> Self {
        Self::FormUrlencoded { fields }
    }

    /// Creates a multipart form-data body.
    #[must_use]
    pub const fn form_data(fields: Vec<FormDataField>) -> Self {
        Self::FormData { fields }
    }

    /// Creates a binary body referencing a file path.
    #[must_use]
    pub fn binary(path: impl Into<String>) -> Self {
        Self::Binary { path: path.into() }
    }

    /// Creates a GraphQL body.
    #[must_use]
    pub fn graphql(query: impl Into<String>, variables: Option<JsonValue>) -> Self {
        Self::Graphql {
            query: query.into(),
            variables,
        }
    }
}

/// A field in a multipart form-data body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[must_use]
    pub fn text(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Text {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Creates a file field.
    #[must_use]
    pub fn file(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self::File {
            name: name.into(),
            path: path.into(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_body_json() {
        let body = PersistenceRequestBody::json(serde_json::json!({"name": "test"}));
        match body {
            PersistenceRequestBody::Json { content } => {
                assert_eq!(content["name"], "test");
            }
            _ => panic!("Expected JSON body"),
        }
    }

    #[test]
    fn test_body_text() {
        let body = PersistenceRequestBody::text("Hello, World!");
        match body {
            PersistenceRequestBody::Text { content } => {
                assert_eq!(content, "Hello, World!");
            }
            _ => panic!("Expected Text body"),
        }
    }

    #[test]
    fn test_form_data_field() {
        let text_field = FormDataField::text("username", "john");
        let file_field = FormDataField::file("avatar", "uploads/avatar.png");

        match text_field {
            FormDataField::Text { name, value } => {
                assert_eq!(name, "username");
                assert_eq!(value, "john");
            }
            FormDataField::File { .. } => panic!("Expected Text field"),
        }

        match file_field {
            FormDataField::File { name, path } => {
                assert_eq!(name, "avatar");
                assert_eq!(path, "uploads/avatar.png");
            }
            FormDataField::Text { .. } => panic!("Expected File field"),
        }
    }
}
