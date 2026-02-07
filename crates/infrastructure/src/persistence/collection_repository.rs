//! File system based collection repository implementation.

use std::path::{Path, PathBuf};

use vortex_application::ports::{
    CollectionError, CollectionRepository, CollectionTree, FileSystem, FolderTree, slugify,
};
use vortex_domain::persistence::{
    CURRENT_SCHEMA_VERSION, PersistenceCollection, PersistenceFolder, SavedRequest,
};

use crate::serialization::{from_json, to_json_stable};

/// File names used in the collection structure.
const COLLECTION_FILE: &str = "collection.json";
const FOLDER_FILE: &str = "folder.json";
const REQUESTS_DIR: &str = "requests";

/// File system based implementation of `CollectionRepository`.
pub struct FileSystemCollectionRepository<F: FileSystem> {
    fs: F,
}

impl<F: FileSystem> FileSystemCollectionRepository<F> {
    /// Creates a new repository with the given file system implementation.
    #[must_use]
    pub const fn new(fs: F) -> Self {
        Self { fs }
    }
}

impl<F: FileSystem + Send + Sync> FileSystemCollectionRepository<F> {
    /// Recursively loads a folder and its contents.
    async fn load_folder_tree(
        &self,
        folder_path: &Path,
        relative_path: &str,
    ) -> Result<FolderTree, CollectionError> {
        let folder_file = folder_path.join(FOLDER_FILE);
        let folder: PersistenceFolder = self.load_json(&folder_file).await?;

        let mut requests = Vec::new();
        let mut subfolders = Vec::new();

        let entries = self
            .fs
            .read_dir(folder_path)
            .await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

        for entry in entries {
            let file_name = entry
                .file_name()
                .and_then(|n| n.to_str())
                .map(ToString::to_string)
                .unwrap_or_default();

            if file_name == FOLDER_FILE {
                continue;
            }

            if self.fs.is_dir(&entry).await {
                // Check if it's a subfolder (has folder.json)
                let subfolder_meta = entry.join(FOLDER_FILE);
                if self.fs.exists(&subfolder_meta).await {
                    let sub_relative = format!("{relative_path}/{file_name}");
                    let subfolder_tree =
                        Box::pin(self.load_folder_tree(&entry, &sub_relative)).await?;
                    subfolders.push(subfolder_tree);
                }
            } else if file_name.ends_with(".json") {
                // It's a request file
                let request = self.load_request(&entry).await?;
                requests.push(request);
            }
        }

        Ok(FolderTree {
            folder,
            requests,
            subfolders,
            path: relative_path.to_string(),
        })
    }

    /// Loads and deserializes a JSON file.
    async fn load_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &Path,
    ) -> Result<T, CollectionError> {
        let content = self
            .fs
            .read_file_string(path)
            .await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;
        from_json(&content).map_err(|e| CollectionError::Serialization(e.to_string()))
    }

    /// Serializes and saves a value to a JSON file.
    async fn save_json<T: serde::Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), CollectionError> {
        let json =
            to_json_stable(value).map_err(|e| CollectionError::Serialization(e.to_string()))?;
        self.fs
            .write_file(path, json.as_bytes())
            .await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))
    }
}

impl<F: FileSystem + Send + Sync> CollectionRepository for FileSystemCollectionRepository<F> {
    async fn load_collection(
        &self,
        collection_dir: &Path,
    ) -> Result<CollectionTree, CollectionError> {
        let collection_file = collection_dir.join(COLLECTION_FILE);

        if !self.fs.exists(&collection_file).await {
            return Err(CollectionError::NotFound(
                collection_dir.display().to_string(),
            ));
        }

        let collection: PersistenceCollection = self.load_json(&collection_file).await?;

        // Validate schema version
        if collection.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(CollectionError::SchemaMismatch {
                expected: CURRENT_SCHEMA_VERSION,
                found: collection.schema_version,
            });
        }

        let requests_dir = collection_dir.join(REQUESTS_DIR);
        let mut requests = Vec::new();
        let mut folders = Vec::new();

        if self.fs.exists(&requests_dir).await {
            let entries = self
                .fs
                .read_dir(&requests_dir)
                .await
                .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

            for entry in entries {
                let file_name = entry
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(ToString::to_string)
                    .unwrap_or_default();

                if self.fs.is_dir(&entry).await {
                    // Check if it's a folder (has folder.json)
                    let folder_meta = entry.join(FOLDER_FILE);
                    if self.fs.exists(&folder_meta).await {
                        let folder_tree = self.load_folder_tree(&entry, &file_name).await?;
                        folders.push(folder_tree);
                    }
                } else if file_name.ends_with(".json") {
                    let request = self.load_request(&entry).await?;
                    requests.push(request);
                }
            }
        }

        Ok(CollectionTree {
            collection,
            requests,
            folders,
        })
    }

    async fn save_collection(
        &self,
        collection_dir: &Path,
        collection: &PersistenceCollection,
    ) -> Result<(), CollectionError> {
        let collection_file = collection_dir.join(COLLECTION_FILE);
        self.save_json(&collection_file, collection).await
    }

    async fn create_collection(
        &self,
        collection_dir: &Path,
        collection: &PersistenceCollection,
    ) -> Result<(), CollectionError> {
        if self.fs.exists(collection_dir).await {
            return Err(CollectionError::InvalidStructure(format!(
                "Directory already exists: {}",
                collection_dir.display()
            )));
        }

        // Create directory structure
        self.fs
            .create_dir_all(collection_dir)
            .await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

        let requests_dir = collection_dir.join(REQUESTS_DIR);
        self.fs
            .create_dir_all(&requests_dir)
            .await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

        // Save collection metadata
        self.save_collection(collection_dir, collection).await
    }

    async fn delete_collection(&self, collection_dir: &Path) -> Result<(), CollectionError> {
        self.fs
            .remove_dir_all(collection_dir)
            .await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))
    }

    async fn load_request(&self, request_path: &Path) -> Result<SavedRequest, CollectionError> {
        if !self.fs.exists(request_path).await {
            return Err(CollectionError::RequestNotFound(
                request_path.display().to_string(),
            ));
        }
        self.load_json(request_path).await
    }

    async fn save_request(
        &self,
        request_path: &Path,
        request: &SavedRequest,
    ) -> Result<(), CollectionError> {
        self.save_json(request_path, request).await
    }

    async fn create_request(
        &self,
        collection_dir: &Path,
        folder_path: Option<&Path>,
        request: &SavedRequest,
    ) -> Result<PathBuf, CollectionError> {
        let base_dir = match folder_path {
            Some(folder) => collection_dir.join(REQUESTS_DIR).join(folder),
            None => collection_dir.join(REQUESTS_DIR),
        };

        // Ensure directory exists
        self.fs
            .create_dir_all(&base_dir)
            .await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

        // Generate filename from request name
        let filename = format!("{}.json", slugify(&request.name));
        let request_path = base_dir.join(&filename);

        // Check for duplicates
        if self.fs.exists(&request_path).await {
            return Err(CollectionError::DuplicateId(
                request_path.display().to_string(),
            ));
        }

        self.save_request(&request_path, request).await?;
        Ok(request_path)
    }

    async fn delete_request(&self, request_path: &Path) -> Result<(), CollectionError> {
        self.fs
            .remove_file(request_path)
            .await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))
    }

    async fn load_folder(&self, folder_path: &Path) -> Result<PersistenceFolder, CollectionError> {
        let folder_file = folder_path.join(FOLDER_FILE);
        if !self.fs.exists(&folder_file).await {
            return Err(CollectionError::FolderNotFound(
                folder_path.display().to_string(),
            ));
        }
        self.load_json(&folder_file).await
    }

    async fn save_folder(
        &self,
        folder_path: &Path,
        folder: &PersistenceFolder,
    ) -> Result<(), CollectionError> {
        let folder_file = folder_path.join(FOLDER_FILE);
        self.save_json(&folder_file, folder).await
    }

    async fn create_folder(
        &self,
        collection_dir: &Path,
        parent_folder: Option<&Path>,
        folder: &PersistenceFolder,
    ) -> Result<PathBuf, CollectionError> {
        let base_dir = match parent_folder {
            Some(parent) => collection_dir.join(REQUESTS_DIR).join(parent),
            None => collection_dir.join(REQUESTS_DIR),
        };

        let folder_name = slugify(&folder.name);
        let folder_path = base_dir.join(&folder_name);

        if self.fs.exists(&folder_path).await {
            return Err(CollectionError::InvalidStructure(format!(
                "Folder already exists: {}",
                folder_path.display()
            )));
        }

        self.fs
            .create_dir_all(&folder_path)
            .await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))?;

        self.save_folder(&folder_path, folder).await?;
        Ok(folder_path)
    }

    async fn delete_folder(&self, folder_path: &Path) -> Result<(), CollectionError> {
        self.fs
            .remove_dir_all(folder_path)
            .await
            .map_err(|e| CollectionError::FileSystem(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use vortex_domain::persistence::PersistenceHttpMethod;

    use super::*;

    #[test]
    fn test_request_serialization() {
        let request = SavedRequest::new(
            "test-id".to_string(),
            "Get Users",
            PersistenceHttpMethod::Get,
            "https://api.example.com/users",
        );

        let json = to_json_stable(&request).expect("serialization should succeed");
        assert!(json.contains("\"name\": \"Get Users\""));
        assert!(json.contains("\"method\": \"GET\""));
    }
}
