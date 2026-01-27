//! Vortex Domain - Core business types
//!
//! This crate defines the domain model for the Vortex API Client.
//! All types here are pure Rust with no I/O dependencies.

pub mod auth;
pub mod collection;
pub mod environment;
pub mod error;
pub mod id;
pub mod persistence;
pub mod request;
pub mod response;
pub mod state;

pub use error::{DomainError, DomainResult};
pub use id::{generate_id, generate_id_v7};
pub use state::{RequestErrorKind, RequestState};
