//! Port definitions (interfaces)
//!
//! Ports define the boundaries between the application core and external systems.
//! Each port is a trait that can be implemented by adapters in the infrastructure layer.

mod clock;
mod collection_repository;
mod environment_repository;
mod file_system;
mod http_client;
mod secrets_repository;
mod storage;
mod workspace_repository;

pub use clock::Clock;
pub use collection_repository::{
    slugify, CollectionError, CollectionRepository, CollectionTree, FolderTree,
};
pub use environment_repository::{EnvironmentError, EnvironmentRepository};
pub use file_system::{FileSystem, FileSystemError};
pub use http_client::{CancellationReceiver, CancellationToken, HttpClient, HttpClientError};
pub use secrets_repository::{SecretsError, SecretsRepository};
pub use storage::{CollectionStorage, EnvironmentStorage};
pub use workspace_repository::{WorkspaceError, WorkspaceRepository};
