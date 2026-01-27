//! Port definitions (interfaces)
//!
//! Ports define the boundaries between the application core and external systems.
//! Each port is a trait that can be implemented by adapters in the infrastructure layer.

mod clock;
mod http_client;
mod storage;

pub use clock::Clock;
pub use http_client::{CancellationReceiver, CancellationToken, HttpClient, HttpClientError};
pub use storage::{CollectionStorage, EnvironmentStorage};
