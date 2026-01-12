//! # Vortex Git Backend
//!
//! Git-based configuration source for Vortex Config Server.
//!
//! This crate provides a Git backend implementation that can clone repositories,
//! read configuration files, and support Spring Cloud Config file conventions.
//!
//! ## Features
//!
//! - Pure Rust Git implementation via `gix`
//! - Async trait-based configuration source abstraction
//! - Support for branches, tags, and commit references
//! - Background refresh with configurable intervals
//! - Spring Cloud Config compatible file resolution
//!
//! ## Example
//!
//! ```ignore
//! use vortex_git::{GitBackend, GitBackendConfig, ConfigSource, ConfigQuery};
//!
//! let config = GitBackendConfig::builder()
//!     .uri("https://github.com/org/config-repo.git")
//!     .local_path("/tmp/config-repo")
//!     .default_label("main")
//!     .build()?;
//!
//! let backend = GitBackend::new(config).await?;
//!
//! let query = ConfigQuery::new("myapp", vec!["dev"]);
//! let result = backend.fetch(&query).await?;
//! ```

pub mod backend;
pub mod error;
pub mod reader;
pub mod repository;
pub mod source;
pub mod sync;

// Re-exports
pub use backend::GitBackend;
pub use error::ConfigSourceError;
pub use reader::{ConfigFileResolver, ConfigFormat, ConfigParser};
pub use repository::{GitBackendConfig, GitRef, GitRepository};
pub use source::{ConfigQuery, ConfigResult, ConfigSource};
pub use sync::{GitState, RefreshConfig, RefreshHandle, RefreshScheduler};
