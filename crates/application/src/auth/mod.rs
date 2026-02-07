//! Authentication module for Vortex API Client.
//!
//! This module provides:
//! - In-memory token storage with expiry tracking
//! - Authentication provider trait for resolving auth configs
//! - `OAuth2` flow state management

mod provider;
mod token_store;

pub use provider::{AuthEvent, AuthProvider, AuthorizationState};
pub use token_store::{TokenStatus, TokenStore};
