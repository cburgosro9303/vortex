//! Vortex Domain - Core business types
//!
//! This crate defines the domain model for the Vortex API Client.
//! All types here are pure Rust with no I/O dependencies.

pub mod auth;
pub mod collection;
pub mod environment;
pub mod error;
pub mod history;
pub mod id;
pub mod persistence;
pub mod request;
pub mod response;
pub mod settings;
pub mod state;

pub use error::{DomainError, DomainResult};
pub use history::{HistoryAuth, HistoryEntry, HistoryHeader, HistoryParam, RequestHistory};
pub use id::{generate_id, generate_id_v7};
pub use settings::{FontScale, ThemeMode, UserSettings};
pub use state::{RequestErrorKind, RequestState};
