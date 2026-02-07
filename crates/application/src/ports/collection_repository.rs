//! Collection repository port.

use std::path::{Path, PathBuf};

use vortex_domain::persistence::{PersistenceCollection, PersistenceFolder, SavedRequest};

/// Error type for collection operations.
#[derive(Debug, thiserror::Error)]
pub enum CollectionError {
    /// Collection not found.
    #[error("Collection not found: {0}")]
    NotFound(String),

    /// Request not found.
    #[error("Request not found: {0}")]
    RequestNotFound(String),

    /// Folder not found.
    #[error("Folder not found: {0}")]
    FolderNotFound(String),

    /// Invalid collection structure.
    #[error("Invalid collection structure: {0}")]
    InvalidStructure(String),

    /// Schema version mismatch.
    #[error("Schema version mismatch: expected {expected}, found {found}")]
    SchemaMismatch {
        /// Expected schema version.
        expected: u32,
        /// Found schema version.
        found: u32,
    },

    /// Duplicate ID.
    #[error("Duplicate ID: {0}")]
    DuplicateId(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// File system error.
    #[error("File system error: {0}")]
    FileSystem(String),
}

/// Represents the full tree structure of a loaded collection.
#[derive(Debug, Clone)]
pub struct CollectionTree {
    /// The collection metadata.
    pub collection: PersistenceCollection,
    /// Root-level requests (in requests/ directory).
    pub requests: Vec<SavedRequest>,
    /// Folders with their nested content.
    pub folders: Vec<FolderTree>,
}

/// A folder with its contents.
#[derive(Debug, Clone)]
pub struct FolderTree {
    /// The folder metadata.
    pub folder: PersistenceFolder,
    /// Requests in this folder.
    pub requests: Vec<SavedRequest>,
    /// Nested subfolders.
    pub subfolders: Vec<FolderTree>,
    /// Relative path from collection root.
    pub path: String,
}

/// Repository for collection and request file operations.
pub trait CollectionRepository: Send + Sync {
    // === Collection Operations ===

    /// Loads a collection and all its contents from disk.
    ///
    /// # Arguments
    /// * `collection_dir` - Path to the collection directory (contains collection.json)
    ///
    /// # Errors
    ///
    /// Returns an error if the collection cannot be loaded.
    fn load_collection(
        &self,
        collection_dir: &Path,
    ) -> impl std::future::Future<Output = Result<CollectionTree, CollectionError>> + Send;

    /// Saves a collection metadata file.
    ///
    /// Only saves the collection.json, not the requests.
    ///
    /// # Errors
    ///
    /// Returns an error if the collection cannot be saved.
    fn save_collection(
        &self,
        collection_dir: &Path,
        collection: &PersistenceCollection,
    ) -> impl std::future::Future<Output = Result<(), CollectionError>> + Send;

    /// Creates a new collection with initial directory structure.
    ///
    /// Creates:
    /// - collection.json
    /// - requests/ directory
    ///
    /// # Errors
    ///
    /// Returns an error if the collection cannot be created.
    fn create_collection(
        &self,
        collection_dir: &Path,
        collection: &PersistenceCollection,
    ) -> impl std::future::Future<Output = Result<(), CollectionError>> + Send;

    /// Deletes a collection and all its contents.
    ///
    /// # Errors
    ///
    /// Returns an error if the collection cannot be deleted.
    fn delete_collection(
        &self,
        collection_dir: &Path,
    ) -> impl std::future::Future<Output = Result<(), CollectionError>> + Send;

    // === Request Operations ===

    /// Loads a single request from disk.
    ///
    /// # Arguments
    /// * `request_path` - Full path to the request JSON file
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be loaded.
    fn load_request(
        &self,
        request_path: &Path,
    ) -> impl std::future::Future<Output = Result<SavedRequest, CollectionError>> + Send;

    /// Saves a request to disk.
    ///
    /// The filename is derived from the request name (slugified) or can be specified.
    ///
    /// # Arguments
    /// * `request_path` - Full path where the request should be saved
    /// * `request` - The request data to save
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be saved.
    fn save_request(
        &self,
        request_path: &Path,
        request: &SavedRequest,
    ) -> impl std::future::Future<Output = Result<(), CollectionError>> + Send;

    /// Creates a new request file in a collection.
    ///
    /// # Arguments
    /// * `collection_dir` - Path to the collection
    /// * `folder_path` - Optional subfolder path (None for root requests/)
    /// * `request` - The request to create
    ///
    /// # Returns
    /// The path where the request was saved
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be created.
    fn create_request(
        &self,
        collection_dir: &Path,
        folder_path: Option<&Path>,
        request: &SavedRequest,
    ) -> impl std::future::Future<Output = Result<PathBuf, CollectionError>> + Send;

    /// Deletes a request file.
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be deleted.
    fn delete_request(
        &self,
        request_path: &Path,
    ) -> impl std::future::Future<Output = Result<(), CollectionError>> + Send;

    // === Folder Operations ===

    /// Loads a folder metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the folder cannot be loaded.
    fn load_folder(
        &self,
        folder_path: &Path,
    ) -> impl std::future::Future<Output = Result<PersistenceFolder, CollectionError>> + Send;

    /// Saves a folder metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the folder cannot be saved.
    fn save_folder(
        &self,
        folder_path: &Path,
        folder: &PersistenceFolder,
    ) -> impl std::future::Future<Output = Result<(), CollectionError>> + Send;

    /// Creates a new folder in a collection.
    ///
    /// # Returns
    /// The path to the created folder
    ///
    /// # Errors
    ///
    /// Returns an error if the folder cannot be created.
    fn create_folder(
        &self,
        collection_dir: &Path,
        parent_folder: Option<&Path>,
        folder: &PersistenceFolder,
    ) -> impl std::future::Future<Output = Result<PathBuf, CollectionError>> + Send;

    /// Deletes a folder and all its contents.
    ///
    /// # Errors
    ///
    /// Returns an error if the folder cannot be deleted.
    fn delete_folder(
        &self,
        folder_path: &Path,
    ) -> impl std::future::Future<Output = Result<(), CollectionError>> + Send;
}

/// Helper to generate a filesystem-safe filename from a name.
#[must_use]
pub fn slugify(name: &str) -> String {
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

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Get Users"), "get-users");
        assert_eq!(slugify("POST /api/v1/users"), "post-api-v1-users");
        assert_eq!(slugify("  Multiple   Spaces  "), "multiple-spaces");
        assert_eq!(slugify("Special!@#$%Chars"), "special-chars");
    }

    #[test]
    fn test_slugify_empty() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("---"), "");
    }
}
