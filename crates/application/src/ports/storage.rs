//! Storage ports

use std::future::Future;
use std::path::Path;

use vortex_domain::{collection::Collection, environment::Environment};

use crate::ApplicationResult;

/// Port for persisting and loading collections.
pub trait CollectionStorage: Send + Sync {
    /// Saves a collection to the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the collection cannot be serialized or written.
    fn save(
        &self,
        collection: &Collection,
        path: &Path,
    ) -> impl Future<Output = ApplicationResult<()>> + Send;

    /// Loads a collection from the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    fn load(&self, path: &Path) -> impl Future<Output = ApplicationResult<Collection>> + Send;

    /// Lists all collections in the specified directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    fn list(
        &self,
        directory: &Path,
    ) -> impl Future<Output = ApplicationResult<Vec<Collection>>> + Send;
}

/// Port for persisting and loading environments.
pub trait EnvironmentStorage: Send + Sync {
    /// Saves an environment to the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the environment cannot be serialized or written.
    fn save(
        &self,
        environment: &Environment,
        path: &Path,
    ) -> impl Future<Output = ApplicationResult<()>> + Send;

    /// Loads an environment from the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    fn load(&self, path: &Path) -> impl Future<Output = ApplicationResult<Environment>> + Send;
}
