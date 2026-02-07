//! Vortex Infrastructure - Adapters and implementations
//!
//! This crate provides concrete implementations of the ports
//! defined in the application layer.

pub mod adapters;
pub mod auth;
pub mod codegen;
pub mod export;
pub mod http;
pub mod import;
pub mod persistence;
pub mod scripting;
pub mod serialization;
pub mod testing;

pub use adapters::ReqwestHttpClient;
pub use auth::OAuth2Provider;
pub use codegen::{CodeGenerator, generate_code};
pub use export::{ExportError, HarExporter, OpenApiExporter, export_request, export_requests};
pub use http::{BodyBuildError, BuiltBody, build_body};
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
pub use scripting::{ParseError, ScriptExecutor, parse_script};
pub use serialization::{
    SerializationError, from_json, from_json_bytes, to_json_stable, to_json_stable_bytes,
};
pub use testing::TestRunner;
