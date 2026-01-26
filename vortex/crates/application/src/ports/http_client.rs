//! HTTP Client port

use std::future::Future;

use vortex_domain::{request::RequestSpec, response::ResponseSpec};

use crate::ApplicationResult;

/// Port for executing HTTP requests.
///
/// This trait abstracts the HTTP client implementation, allowing
/// the application layer to be independent of specific HTTP libraries.
pub trait HttpClient: Send + Sync {
    /// Executes an HTTP request and returns the response.
    ///
    /// # Arguments
    ///
    /// * `request` - The request specification to execute
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails due to network issues,
    /// timeout, or other HTTP-related problems.
    fn execute(
        &self,
        request: &RequestSpec,
    ) -> impl Future<Output = ApplicationResult<ResponseSpec>> + Send;

    /// Cancels any pending request with the given ID.
    ///
    /// This is a best-effort operation; the request may still complete
    /// if it was already in flight.
    fn cancel(&self, request_id: uuid::Uuid) -> impl Future<Output = ()> + Send;
}
