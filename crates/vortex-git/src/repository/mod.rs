//! Git repository management.
//!
//! This module provides functionality for cloning and updating Git repositories.

mod config;
mod git_ops;
mod refs;

pub use config::GitBackendConfig;
pub use git_ops::GitRepository;
pub use refs::GitRef;
