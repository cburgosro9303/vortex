//! Vortex Infrastructure - Adapters and implementations
//!
//! This crate provides concrete implementations of the ports
//! defined in the application layer.

pub mod adapters;
pub mod import;
pub mod persistence;
pub mod serialization;

pub use adapters::ReqwestHttpClient;
pub use import::{
    ImportConfig, ImportError, ImportFormat, ImportPreview, ImportResult, ImportWarning,
    PostmanCollection, PostmanEnvironment, PostmanImporter, ValidationResult, WarningSeverity,
    WarningStats,
};
pub use persistence::{
    FileEnvironmentRepository, FileSecretsRepository, FileSystemCollectionRepository,
    FileSystemWorkspaceRepository, HistoryError, HistoryRepository, SettingsError,
    SettingsRepository, TokioFileSystem,
};
pub use serialization::{
    from_json, from_json_bytes, to_json_stable, to_json_stable_bytes, SerializationError,
};
