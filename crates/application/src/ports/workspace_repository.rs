//! Workspace repository port.

use std::path::Path;

use vortex_domain::persistence::WorkspaceManifest;

/// Error type for workspace operations.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    /// Workspace not found.
    #[error("Workspace not found at: {0}")]
    NotFound(String),

    /// Invalid workspace.
    #[error("Invalid workspace: {0}")]
    Invalid(String),

    /// Workspace already exists.
    #[error("Workspace already exists at: {0}")]
    AlreadyExists(String),

    /// Schema version mismatch.
    #[error("Schema version mismatch: expected {expected}, found {found}")]
    SchemaMismatch {
        /// Expected schema version.
        expected: u32,
        /// Found schema version.
        found: u32,
    },

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// File system error.
    #[error("File system error: {0}")]
    FileSystem(String),
}

/// Repository for workspace manifest operations.
pub trait WorkspaceRepository: Send + Sync {
    /// Loads a workspace manifest from a directory.
    ///
    /// The directory must contain a `vortex.json` file.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace cannot be loaded.
    fn load(
        &self,
        workspace_dir: &Path,
    ) -> impl std::future::Future<Output = Result<WorkspaceManifest, WorkspaceError>> + Send;

    /// Saves a workspace manifest to a directory.
    ///
    /// Creates `vortex.json` in the specified directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace cannot be saved.
    fn save(
        &self,
        workspace_dir: &Path,
        manifest: &WorkspaceManifest,
    ) -> impl std::future::Future<Output = Result<(), WorkspaceError>> + Send;

    /// Creates a new workspace with initial structure.
    ///
    /// Creates:
    /// - vortex.json
    /// - collections/ directory
    /// - environments/ directory
    /// - .vortex/ directory
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace cannot be created.
    fn create(
        &self,
        workspace_dir: &Path,
        name: &str,
    ) -> impl std::future::Future<Output = Result<WorkspaceManifest, WorkspaceError>> + Send;

    /// Checks if a directory contains a valid workspace.
    fn is_workspace(&self, path: &Path) -> impl std::future::Future<Output = bool> + Send;
}
