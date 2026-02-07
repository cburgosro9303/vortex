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
    #[must_use]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_manifest_new() {
        let manifest = WorkspaceManifest::new("My Workspace");
        assert_eq!(manifest.name, "My Workspace");
        assert_eq!(manifest.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(manifest.collections.is_empty());
        assert!(manifest.settings.is_some());
    }

    #[test]
    fn test_workspace_manifest_add_collection() {
        let mut manifest = WorkspaceManifest::new("Test");
        manifest.add_collection("collections/api-v1");
        manifest.add_collection("collections/api-v2");
        assert_eq!(manifest.collections.len(), 2);
    }
}
