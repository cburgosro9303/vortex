//! Save collection use case.

use std::path::PathBuf;

use vortex_domain::persistence::PersistenceCollection;

use crate::ports::{CollectionError, CollectionRepository, FileSystem};

/// Input for saving a collection.
#[derive(Debug, Clone)]
pub struct SaveCollectionInput {
    /// Path to the collection directory.
    pub collection_dir: PathBuf,
    /// The collection metadata to save.
    pub collection: PersistenceCollection,
    /// Whether to create the collection if it doesn't exist.
    pub create_if_missing: bool,
}

/// Use case for saving a collection to disk.
pub struct SaveCollection<R: CollectionRepository, F: FileSystem> {
    collection_repo: R,
    fs: F,
}

impl<R: CollectionRepository, F: FileSystem> SaveCollection<R, F> {
    /// Creates a new `SaveCollection` use case.
    #[must_use]
    pub const fn new(collection_repo: R, fs: F) -> Self {
        Self {
            collection_repo,
            fs,
        }
    }

    /// Saves the collection metadata to disk.
    ///
    /// If `create_if_missing` is true and the collection doesn't exist,
    /// creates the full directory structure.
    ///
    /// # Errors
    /// - Returns error if collection doesn't exist and `create_if_missing` is false
    /// - Returns error if file system operations fail
    pub async fn execute(&self, input: SaveCollectionInput) -> Result<(), CollectionError> {
        let collection_file = input.collection_dir.join("collection.json");

        // Check if this is a new collection
        let exists = self.fs.exists(&collection_file).await;

        if !exists && input.create_if_missing {
            self.collection_repo
                .create_collection(&input.collection_dir, &input.collection)
                .await
        } else if !exists {
            Err(CollectionError::NotFound(
                input.collection_dir.display().to_string(),
            ))
        } else {
            self.collection_repo
                .save_collection(&input.collection_dir, &input.collection)
                .await
        }
    }
}
