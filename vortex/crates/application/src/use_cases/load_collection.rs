//! Load collection use case.

use std::path::PathBuf;

use crate::ports::{CollectionError, CollectionRepository, CollectionTree};

/// Input for loading a collection.
#[derive(Debug, Clone)]
pub struct LoadCollectionInput {
    /// Path to the collection directory.
    pub collection_dir: PathBuf,
}

/// Use case for loading a collection from disk.
pub struct LoadCollection<R: CollectionRepository> {
    collection_repo: R,
}

impl<R: CollectionRepository> LoadCollection<R> {
    /// Creates a new `LoadCollection` use case.
    #[must_use]
    pub const fn new(collection_repo: R) -> Self {
        Self { collection_repo }
    }

    /// Loads a collection and all its contents from disk.
    ///
    /// # Returns
    /// A `CollectionTree` containing the collection metadata,
    /// all requests, and nested folder structure.
    ///
    /// # Errors
    /// - Returns error if collection doesn't exist
    /// - Returns error if schema version is unsupported
    /// - Returns error if JSON parsing fails
    pub async fn execute(
        &self,
        input: LoadCollectionInput,
    ) -> Result<CollectionTree, CollectionError> {
        self.collection_repo
            .load_collection(&input.collection_dir)
            .await
    }
}
