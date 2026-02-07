//! Collection metadata type (collection.json).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::auth::PersistenceAuth;
use super::common::{CURRENT_SCHEMA_VERSION, Id};

/// Collection metadata stored in `collection.json` within a collection directory.
///
/// A collection groups related requests and can define shared authentication
/// and variables that are inherited by all requests within.
///
/// Fields are ordered alphabetically for deterministic serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistenceCollection {
    /// Authentication inherited by all requests in this collection.
    /// Can be overridden at folder or request level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<PersistenceAuth>,

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

impl PersistenceCollection {
    /// Creates a new collection with a generated UUID.
    #[must_use]
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
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the collection-level authentication.
    #[must_use]
    pub fn with_auth(mut self, auth: PersistenceAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Adds a variable to the collection.
    #[must_use]
    pub fn with_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(key.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_new() {
        let collection = PersistenceCollection::new(
            "550e8400-e29b-41d4-a716-446655440000".to_string(),
            "My API",
        );
        assert_eq!(collection.name, "My API");
        assert_eq!(collection.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(collection.auth.is_none());
        assert!(collection.description.is_none());
    }

    #[test]
    fn test_collection_with_builders() {
        let collection = PersistenceCollection::new("test-id".to_string(), "Test Collection")
            .with_description("A test collection")
            .with_auth(PersistenceAuth::bearer("token"))
            .with_variable("base_url", "https://api.example.com");

        assert_eq!(
            collection.description,
            Some("A test collection".to_string())
        );
        assert!(collection.auth.is_some());
        assert_eq!(
            collection.variables.get("base_url"),
            Some(&"https://api.example.com".to_string())
        );
    }
}
