//! File system based workspace repository implementation.

use std::path::Path;

use vortex_application::ports::{FileSystem, WorkspaceError, WorkspaceRepository};
use vortex_domain::persistence::{CURRENT_SCHEMA_VERSION, WorkspaceManifest};

use crate::serialization::{from_json, to_json_stable};

const WORKSPACE_FILE: &str = "vortex.json";
const COLLECTIONS_DIR: &str = "collections";
const ENVIRONMENTS_DIR: &str = "environments";
const VORTEX_DIR: &str = ".vortex";

/// File system based implementation of `WorkspaceRepository`.
pub struct FileSystemWorkspaceRepository<F: FileSystem> {
    fs: F,
}

impl<F: FileSystem> FileSystemWorkspaceRepository<F> {
    /// Creates a new repository with the given file system implementation.
    #[must_use]
    pub const fn new(fs: F) -> Self {
        Self { fs }
    }
}

impl<F: FileSystem + Send + Sync> WorkspaceRepository for FileSystemWorkspaceRepository<F> {
    async fn load(&self, workspace_dir: &Path) -> Result<WorkspaceManifest, WorkspaceError> {
        let manifest_path = workspace_dir.join(WORKSPACE_FILE);

        if !self.fs.exists(&manifest_path).await {
            return Err(WorkspaceError::NotFound(
                workspace_dir.display().to_string(),
            ));
        }

        let content = self
            .fs
            .read_file_string(&manifest_path)
            .await
            .map_err(|e| WorkspaceError::FileSystem(e.to_string()))?;

        let manifest: WorkspaceManifest =
            from_json(&content).map_err(|e| WorkspaceError::Serialization(e.to_string()))?;

        // Validate schema version
        if manifest.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(WorkspaceError::SchemaMismatch {
                expected: CURRENT_SCHEMA_VERSION,
                found: manifest.schema_version,
            });
        }

        Ok(manifest)
    }

    async fn save(
        &self,
        workspace_dir: &Path,
        manifest: &WorkspaceManifest,
    ) -> Result<(), WorkspaceError> {
        let manifest_path = workspace_dir.join(WORKSPACE_FILE);

        let json =
            to_json_stable(manifest).map_err(|e| WorkspaceError::Serialization(e.to_string()))?;

        self.fs
            .write_file(&manifest_path, json.as_bytes())
            .await
            .map_err(|e| WorkspaceError::FileSystem(e.to_string()))
    }

    async fn create(
        &self,
        workspace_dir: &Path,
        name: &str,
    ) -> Result<WorkspaceManifest, WorkspaceError> {
        if self.fs.exists(&workspace_dir.join(WORKSPACE_FILE)).await {
            return Err(WorkspaceError::AlreadyExists(
                workspace_dir.display().to_string(),
            ));
        }

        // Create directory structure
        self.fs
            .create_dir_all(workspace_dir)
            .await
            .map_err(|e| WorkspaceError::FileSystem(e.to_string()))?;

        self.fs
            .create_dir_all(&workspace_dir.join(COLLECTIONS_DIR))
            .await
            .map_err(|e| WorkspaceError::FileSystem(e.to_string()))?;

        self.fs
            .create_dir_all(&workspace_dir.join(ENVIRONMENTS_DIR))
            .await
            .map_err(|e| WorkspaceError::FileSystem(e.to_string()))?;

        self.fs
            .create_dir_all(&workspace_dir.join(VORTEX_DIR))
            .await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workspace_manifest_serialization() {
        let manifest = WorkspaceManifest::new("Test Workspace");
        let json = to_json_stable(&manifest).expect("serialization should succeed");

        assert!(json.contains("\"name\": \"Test Workspace\""));
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.ends_with('\n'));
    }
}
