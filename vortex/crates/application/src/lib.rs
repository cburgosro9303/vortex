//! Vortex Application - Use cases and ports
//!
//! This crate defines the application layer with:
//! - Port traits (interfaces for external dependencies)
//! - Use case orchestration
//! - Application-level error handling

pub mod error;
pub mod ports;

pub use error::{ApplicationError, ApplicationResult};
