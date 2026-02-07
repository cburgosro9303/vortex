//! File-based secrets repository implementation.
//!
//! Secrets are stored in `.vortex/secrets.json` within the workspace.
//! This file should be added to `.gitignore` to prevent accidental commits.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use vortex_application::ports::{FileSystem, FileSystemError, SecretsError, SecretsRepository};
use vortex_domain::environment::SecretsStore;

use crate::serialization::{from_json_bytes, to_json_stable_bytes};

/// Converts FileSystemError to std::io::Error for SecretsError.
fn to_io_error(e: FileSystemError) -> std::io::Error {
    match e {
        FileSystemError::Io(io_err) => io_err,
        FileSystemError::NotFound(path) => {
            std::io::Error::new(std::io::ErrorKind::NotFound, path.display().to_string())
        }
        FileSystemError::PermissionDenied(path) => std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            path.display().to_string(),
        ),
        FileSystemError::AlreadyExists(path) => std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            path.display().to_string(),
        ),
        _ => std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
    }
}

/// File-based secrets repository.
///
/// Stores secrets in `.vortex/secrets.json`:
/// ```json
/// {
///   "schema_version": 1,
///   "secrets": {
///     "development": {
///       "api_key": "sk-dev-123",
///       "db_password": "password123"
///     },
///     "production": {
///       "api_key": "sk-prod-456"
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct FileSecretsRepository<F> {
    fs: F,
}

impl<F: FileSystem> FileSecretsRepository<F> {
    /// Creates a new file-based secrets repository.
    pub const fn new(fs: F) -> Self {
        Self { fs }
    }

    /// Returns the secrets file path for a workspace.
    fn secrets_path(workspace: &Path) -> PathBuf {
        workspace.join(".vortex").join("secrets.json")
    }
}

#[async_trait]
impl<F: FileSystem + Sync> SecretsRepository for FileSecretsRepository<F> {
    async fn load(&self, workspace: &Path) -> Result<SecretsStore, SecretsError> {
        let path = Self::secrets_path(workspace);

        if !self.fs.exists(&path).await {
            // Return empty store if file doesn't exist
            return Ok(SecretsStore::new());
        }

        let content = self
            .fs
            .read_file(&path)
            .await
            .map_err(|e| SecretsError::Io(to_io_error(e)))?;

        let store: SecretsStore =
            from_json_bytes(&content).map_err(|e| SecretsError::Serialization(e.to_string()))?;

        Ok(store)
    }

    async fn save(&self, workspace: &Path, secrets: &SecretsStore) -> Result<(), SecretsError> {
        let vortex_dir = workspace.join(".vortex");
        let path = Self::secrets_path(workspace);

        // Ensure .vortex directory exists
        self.fs
            .create_dir_all(&vortex_dir)
            .await
            .map_err(|e| SecretsError::Io(to_io_error(e)))?;

        let content = to_json_stable_bytes(secrets)
            .map_err(|e| SecretsError::Serialization(e.to_string()))?;

        self.fs
            .write_file(&path, &content)
            .await
            .map_err(|e| SecretsError::Io(to_io_error(e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokioFileSystem;

    #[test]
    fn test_secrets_path() {
        let workspace = PathBuf::from("/test/workspace");
        let path = FileSecretsRepository::<TokioFileSystem>::secrets_path(&workspace);
        assert_eq!(path, PathBuf::from("/test/workspace/.vortex/secrets.json"));
    }
}
