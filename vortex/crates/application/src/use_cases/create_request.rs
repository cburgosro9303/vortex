//! Create request use case.

use std::path::PathBuf;

use vortex_domain::persistence::SavedRequest;

use crate::ports::{CollectionError, CollectionRepository};

/// Input for creating a new request.
#[derive(Debug, Clone)]
pub struct CreateRequestInput {
    /// Path to the collection directory.
    pub collection_dir: PathBuf,
    /// Optional folder path within the collection (relative to requests/).
    pub folder_path: Option<PathBuf>,
    /// The request to create.
    pub request: SavedRequest,
}

/// Output from creating a request.
#[derive(Debug, Clone)]
pub struct CreateRequestOutput {
    /// The path where the request was saved.
    pub request_path: PathBuf,
    /// The saved request (with any modifications).
    pub request: SavedRequest,
}

/// Use case for creating a new request in a collection.
pub struct CreateRequest<R: CollectionRepository> {
    collection_repo: R,
}

impl<R: CollectionRepository> CreateRequest<R> {
    /// Creates a new `CreateRequest` use case.
    #[must_use]
    pub const fn new(collection_repo: R) -> Self {
        Self { collection_repo }
    }

    /// Creates a new request file in the collection.
    ///
    /// The request will be saved as `{slugified-name}.json` in the
    /// appropriate directory (requests/ or requests/{folder}/).
    ///
    /// # Errors
    /// - Returns error if a request with the same name already exists
    /// - Returns error if the collection or folder doesn't exist
    /// - Returns error if file system operations fail
    pub async fn execute(
        &self,
        input: CreateRequestInput,
    ) -> Result<CreateRequestOutput, CollectionError> {
        let request_path = self
            .collection_repo
            .create_request(
                &input.collection_dir,
                input.folder_path.as_deref(),
                &input.request,
            )
            .await?;

        Ok(CreateRequestOutput {
            request_path,
            request: input.request,
        })
    }
}
