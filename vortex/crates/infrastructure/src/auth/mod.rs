//! Authentication infrastructure adapters.
//!
//! This module provides concrete implementations of authentication providers:
//! - OAuth2 Client Credentials flow
//! - OAuth2 Authorization Code flow (with callback server)

mod oauth2_provider;

pub use oauth2_provider::OAuth2Provider;
