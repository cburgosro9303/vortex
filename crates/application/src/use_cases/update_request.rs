//! Update request use case.

use std::path::PathBuf;

use vortex_domain::persistence::SavedRequest;

use crate::ports::{CollectionError, CollectionRepository, slugify};

/// Input for updating a request.
#[derive(Debug, Clone)]
pub struct UpdateRequestInput {
    /// Current path to the request file.
    pub request_path: PathBuf,
    /// The updated request data.
    pub request: SavedRequest,
    /// Whether to rename the file if the request name changed.
    pub rename_file: bool,
}

/// Output from updating a request.
#[derive(Debug, Clone)]
pub struct UpdateRequestOutput {
    /// The path where the request is now saved (may differ if renamed).
    pub request_path: PathBuf,
}

/// Use case for updating an existing request.
pub struct UpdateRequest<R: CollectionRepository> {
    collection_repo: R,
}

impl<R: CollectionRepository> UpdateRequest<R> {
    /// Creates a new `UpdateRequest` use case.
    #[must_use]
    pub const fn new(collection_repo: R) -> Self {
        Self { collection_repo }
    }

    /// Updates a request file on disk.
    ///
    /// If `rename_file` is true and the request name has changed,
    /// the file will be renamed to match the new name.
    ///
    /// # Errors
    /// - Returns error if the request file doesn't exist
    /// - Returns error if file rename conflicts with existing file
    /// - Returns error if file system operations fail
    pub async fn execute(
        &self,
        input: UpdateRequestInput,
    ) -> Result<UpdateRequestOutput, CollectionError> {
        // Determine if we need to rename
        let current_stem = input
            .request_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let new_stem = slugify(&input.request.name);

        let final_path = if input.rename_file && current_stem != new_stem {
            // Need to rename the file
            let parent = input
                .request_path
                .parent()
                .ok_or_else(|| CollectionError::InvalidStructure("Invalid request path".into()))?;
            let new_path = parent.join(format!("{new_stem}.json"));

            // Delete old file, save to new location
            self.collection_repo
                .delete_request(&input.request_path)
                .await?;
            self.collection_repo
                .save_request(&new_path, &input.request)
                .await?;
            new_path
        } else {
            // Save in place
            self.collection_repo
                .save_request(&input.request_path, &input.request)
                .await?;
            input.request_path
        };

        Ok(UpdateRequestOutput {
            request_path: final_path,
        })
    }
}
