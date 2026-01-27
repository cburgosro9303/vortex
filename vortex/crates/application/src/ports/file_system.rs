//! File system abstraction port.

use std::path::{Path, PathBuf};

/// Error type for file system operations.
#[derive(Debug, thiserror::Error)]
pub enum FileSystemError {
    /// File not found.
    #[error("File not found: {0}")]
    NotFound(PathBuf),

    /// Permission denied.
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    /// Path is not a directory.
    #[error("Path is not a directory: {0}")]
    NotADirectory(PathBuf),

    /// Path is not a file.
    #[error("Path is not a file: {0}")]
    NotAFile(PathBuf),

    /// Path already exists.
    #[error("Path already exists: {0}")]
    AlreadyExists(PathBuf),

    /// Invalid path.
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Abstraction over file system operations.
///
/// This trait allows mocking file system access in tests.
pub trait FileSystem: Send + Sync {
    /// Reads a file's contents as bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    fn read_file(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, FileSystemError>> + Send;

    /// Reads a file's contents as a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is not valid UTF-8.
    fn read_file_string(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<String, FileSystemError>> + Send;

    /// Writes bytes to a file, creating it if necessary.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    fn write_file(
        &self,
        path: &Path,
        contents: &[u8],
    ) -> impl std::future::Future<Output = Result<(), FileSystemError>> + Send;

    /// Creates a directory and all parent directories.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    fn create_dir_all(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<(), FileSystemError>> + Send;

    /// Checks if a path exists.
    fn exists(&self, path: &Path) -> impl std::future::Future<Output = bool> + Send;

    /// Checks if a path is a directory.
    fn is_dir(&self, path: &Path) -> impl std::future::Future<Output = bool> + Send;

    /// Checks if a path is a file.
    fn is_file(&self, path: &Path) -> impl std::future::Future<Output = bool> + Send;

    /// Lists entries in a directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    fn read_dir(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<Vec<PathBuf>, FileSystemError>> + Send;

    /// Removes a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be removed.
    fn remove_file(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<(), FileSystemError>> + Send;

    /// Removes a directory and all its contents.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be removed.
    fn remove_dir_all(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<(), FileSystemError>> + Send;

    /// Copies a file from source to destination.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be copied.
    fn copy_file(
        &self,
        from: &Path,
        to: &Path,
    ) -> impl std::future::Future<Output = Result<(), FileSystemError>> + Send;

    /// Renames/moves a file or directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be renamed.
    fn rename(
        &self,
        from: &Path,
        to: &Path,
    ) -> impl std::future::Future<Output = Result<(), FileSystemError>> + Send;
}
