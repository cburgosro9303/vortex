//! Persistence implementations for file-based storage.

mod collection_repository;
mod file_system;
mod workspace_repository;

pub use collection_repository::*;
pub use file_system::*;
pub use workspace_repository::*;
