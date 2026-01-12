//! Vortex Core - Domain types and traits
//!
//! This crate provides the foundational types for the Vortex Config server.
//!
//! # Key Types
//!
//! - [`ConfigMap`]: Complete configuration for an application
//! - [`PropertySource`]: Configuration from a single source
//! - [`Application`], [`Profile`], [`Label`]: Identifiers for configuration
//! - [`VortexError`]: Main error type
//! - [`Result`]: Type alias for `Result<T, VortexError>`

mod config;
mod error;
mod types;

// Re-export public types
pub use config::{ConfigMap, ConfigMapBuilder, PropertySource};
pub use error::{Result, VortexError};
pub use types::{Application, Label, Profile};

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_defined() {
        assert!(!version().is_empty());
    }

    #[test]
    fn version_is_semver() {
        let v = version();
        assert_eq!(v.split('.').count(), 3, "Version should be semver");
    }

    #[test]
    fn crate_compiles() {
        // Test implícito: si este test corre, el crate compila
        // Verificamos que la función version existe y retorna algo válido
        let v = version();
        assert!(!v.is_empty(), "Version should not be empty");
    }
}
