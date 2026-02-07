//! Real file system implementation.

use std::path::{Path, PathBuf};

use tokio::fs;
use vortex_application::ports::{FileSystem, FileSystemError};

/// Real file system implementation using `tokio::fs`.
#[derive(Debug, Clone, Default)]
pub struct TokioFileSystem;

impl TokioFileSystem {
    /// Creates a new `TokioFileSystem`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl FileSystem for TokioFileSystem {
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>, FileSystemError> {
        fs::read(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FileSystemError::NotFound(path.to_path_buf())
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                FileSystemError::PermissionDenied(path.to_path_buf())
            } else {
                FileSystemError::Io(e)
            }
        })
    }

    async fn read_file_string(&self, path: &Path) -> Result<String, FileSystemError> {
        fs::read_to_string(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FileSystemError::NotFound(path.to_path_buf())
            } else {
                FileSystemError::Io(e)
            }
        })
    }

    async fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), FileSystemError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, contents).await.map_err(FileSystemError::Io)
    }

    async fn create_dir_all(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::create_dir_all(path).await.map_err(FileSystemError::Io)
    }

    async fn exists(&self, path: &Path) -> bool {
        fs::metadata(path).await.is_ok()
    }

    async fn is_dir(&self, path: &Path) -> bool {
        fs::metadata(path).await.is_ok_and(|m| m.is_dir())
    }

    async fn is_file(&self, path: &Path) -> bool {
        fs::metadata(path).await.is_ok_and(|m| m.is_file())
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FileSystemError> {
        let mut entries = Vec::new();
        let mut dir = fs::read_dir(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FileSystemError::NotFound(path.to_path_buf())
            } else {
                FileSystemError::Io(e)
            }
        })?;

        while let Some(entry) = dir.next_entry().await? {
            entries.push(entry.path());
        }

        entries.sort(); // Deterministic ordering
        Ok(entries)
    }

    async fn remove_file(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::remove_file(path).await.map_err(FileSystemError::Io)
    }

    async fn remove_dir_all(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::remove_dir_all(path).await.map_err(FileSystemError::Io)
    }

    async fn copy_file(&self, from: &Path, to: &Path) -> Result<(), FileSystemError> {
        fs::copy(from, to).await?;
        Ok(())
    }

    async fn rename(&self, from: &Path, to: &Path) -> Result<(), FileSystemError> {
        fs::rename(from, to).await.map_err(FileSystemError::Io)
    }
}
