//! File-based environment repository implementation.
//!
//! Environments are stored as JSON files in the `environments/` directory within the workspace.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use vortex_application::ports::{
    EnvironmentError, EnvironmentRepository, FileSystem, FileSystemError,
};
use vortex_domain::environment::Environment;

use crate::serialization::{from_json_bytes, to_json_stable_bytes};

/// Converts FileSystemError to std::io::Error for EnvironmentError.
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

/// File-based environment repository.
///
/// Stores environments as:
/// ```text
/// workspace/
///   environments/
///     development.json
///     staging.json
///     production.json
/// ```
#[derive(Debug, Clone)]
pub struct FileEnvironmentRepository<F> {
    fs: F,
}

impl<F: FileSystem> FileEnvironmentRepository<F> {
    /// Creates a new file-based environment repository.
    pub const fn new(fs: F) -> Self {
        Self { fs }
    }

    /// Returns the environments directory path for a workspace.
    fn environments_dir(workspace: &Path) -> PathBuf {
        workspace.join("environments")
    }

    /// Returns the file path for a specific environment.
    fn environment_path(workspace: &Path, name: &str) -> PathBuf {
        Self::environments_dir(workspace).join(format!("{}.json", slugify(name)))
    }
}

#[async_trait]
impl<F: FileSystem + Sync> EnvironmentRepository for FileEnvironmentRepository<F> {
    async fn load(&self, workspace: &Path, name: &str) -> Result<Environment, EnvironmentError> {
        let path = Self::environment_path(workspace, name);

        if !self.fs.exists(&path).await {
            return Err(EnvironmentError::NotFound(name.to_string()));
        }

        let content = self
            .fs
            .read_file(&path)
            .await
            .map_err(|e| EnvironmentError::Io(to_io_error(e)))?;

        let environment: Environment = from_json_bytes(&content)
            .map_err(|e| EnvironmentError::Serialization(e.to_string()))?;

        Ok(environment)
    }

    async fn save(
        &self,
        workspace: &Path,
        environment: &Environment,
    ) -> Result<(), EnvironmentError> {
        let environments_dir = Self::environments_dir(workspace);

        // Ensure environments directory exists
        self.fs
            .create_dir_all(&environments_dir)
            .await
            .map_err(|e| EnvironmentError::Io(to_io_error(e)))?;

        let path = Self::environment_path(workspace, &environment.name);

        let content = to_json_stable_bytes(environment)
            .map_err(|e| EnvironmentError::Serialization(e.to_string()))?;

        self.fs
            .write_file(&path, &content)
            .await
            .map_err(|e| EnvironmentError::Io(to_io_error(e)))?;

        Ok(())
    }

    async fn list(&self, workspace: &Path) -> Result<Vec<String>, EnvironmentError> {
        let environments_dir = Self::environments_dir(workspace);

        if !self.fs.exists(&environments_dir).await {
            return Ok(Vec::new());
        }

        let entries = self
            .fs
            .read_dir(&environments_dir)
            .await
            .map_err(|e| EnvironmentError::Io(to_io_error(e)))?;

        let mut names = Vec::new();
        for entry in entries {
            if let Some(stem) = entry.file_stem() {
                if entry.extension().is_some_and(|ext| ext == "json") {
                    names.push(stem.to_string_lossy().into_owned());
                }
            }
        }

        names.sort();
        Ok(names)
    }

    async fn delete(&self, workspace: &Path, name: &str) -> Result<(), EnvironmentError> {
        let path = Self::environment_path(workspace, name);

        if !self.fs.exists(&path).await {
            return Err(EnvironmentError::NotFound(name.to_string()));
        }

        self.fs
            .remove_file(&path)
            .await
            .map_err(|e| EnvironmentError::Io(to_io_error(e)))?;

        Ok(())
    }
}

/// Converts a name to a slug suitable for file names.
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokioFileSystem;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Development"), "development");
        assert_eq!(slugify("My Environment"), "my-environment");
        assert_eq!(slugify("Test-123"), "test-123");
        assert_eq!(slugify("a/b\\c:d"), "a-b-c-d");
    }

    #[test]
    fn test_environment_path() {
        let workspace = PathBuf::from("/test/workspace");
        let path = FileEnvironmentRepository::<TokioFileSystem>::environment_path(
            &workspace,
            "Development",
        );
        assert_eq!(
            path,
            PathBuf::from("/test/workspace/environments/development.json")
        );
    }

    #[test]
    fn test_environments_dir() {
        let workspace = PathBuf::from("/test/workspace");
        let dir = FileEnvironmentRepository::<TokioFileSystem>::environments_dir(&workspace);
        assert_eq!(dir, PathBuf::from("/test/workspace/environments"));
    }
}
