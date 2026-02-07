//! Folder metadata type (folder.json).

use serde::{Deserialize, Serialize};

use super::auth::PersistenceAuth;
use super::common::{CURRENT_SCHEMA_VERSION, Id};

/// Folder metadata stored in `folder.json` within a folder directory.
///
/// Folders organize requests hierarchically within a collection.
/// They can define their own auth that overrides collection-level auth.
///
/// Fields are ordered alphabetically for deterministic serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistenceFolder {
    /// Authentication inherited by all requests in this folder.
    /// Overrides collection-level auth if set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<PersistenceAuth>,

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

impl PersistenceFolder {
    /// Creates a new folder with the given ID and name.
    #[must_use]
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
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the folder-level authentication.
    #[must_use]
    pub fn with_auth(mut self, auth: PersistenceAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Sets the explicit ordering of items in this folder.
    #[must_use]
    pub fn with_order(mut self, order: Vec<String>) -> Self {
        self.order = order;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_folder_new() {
        let folder = PersistenceFolder::new("folder-id".to_string(), "Auth Endpoints");
        assert_eq!(folder.name, "Auth Endpoints");
        assert_eq!(folder.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(folder.order.is_empty());
    }

    #[test]
    fn test_folder_with_order() {
        let folder = PersistenceFolder::new("folder-id".to_string(), "Users")
            .with_order(vec!["login.json".to_string(), "logout.json".to_string()]);

        assert_eq!(folder.order.len(), 2);
        assert_eq!(folder.order[0], "login.json");
    }
}
