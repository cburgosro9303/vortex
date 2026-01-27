//! Execute Request Use Case
//!
//! This is the primary use case for Sprint 01: executing an HTTP request
//! and returning the response or error.

use std::sync::Arc;

use thiserror::Error;
use vortex_domain::{RequestErrorKind, RequestState, request::RequestSpec, response::ResponseSpec};

use crate::ports::{CancellationReceiver, HttpClient, HttpClientError};

/// Result type for request execution.
pub type ExecuteResult = Result<ResponseSpec, ExecuteRequestError>;

/// Error type for the execute request use case.
#[derive(Debug, Clone, Error)]
pub enum ExecuteRequestError {
    /// URL is empty.
    #[error("URL is required")]
    EmptyUrl,

    /// URL is invalid.
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// HTTP request failed.
    #[error("{0}")]
    HttpError(#[from] HttpClientError),
}

impl ExecuteRequestError {
    /// Converts this error to a `RequestState::Error` for UI display.
    #[must_use]
    pub fn to_request_state(&self) -> RequestState {
        match self {
            Self::EmptyUrl => RequestState::error(RequestErrorKind::InvalidUrl, "URL is required"),
            Self::InvalidUrl(msg) => RequestState::error(RequestErrorKind::InvalidUrl, msg.clone()),
            Self::HttpError(e) => RequestState::error_with_details(
                e.to_error_kind(),
                e.to_error_kind().title(),
                e.to_string(),
            ),
        }
    }
}

/// Use case for executing HTTP requests.
///
/// This struct encapsulates the business logic for sending requests
/// and handling responses. It uses the `HttpClient` port for actual
/// HTTP communication.
///
/// # Example
///
/// ```ignore
/// let http_client = ReqwestHttpClient::new();
/// let use_case = ExecuteRequest::new(Arc::new(http_client));
///
/// let request = RequestSpec::get("https://api.example.com/users");
/// let response = use_case.execute(&request).await?;
/// ```
pub struct ExecuteRequest<C: HttpClient> {
    client: Arc<C>,
}

impl<C: HttpClient> ExecuteRequest<C> {
    /// Creates a new `ExecuteRequest` use case with the given HTTP client.
    pub fn new(client: Arc<C>) -> Self {
        Self { client }
    }

    /// Executes the request and returns the result.
    ///
    /// # Validation
    ///
    /// - URL must not be empty
    /// - URL must start with http:// or https://
    ///
    /// # Errors
    ///
    /// Returns `ExecuteRequestError` on validation or HTTP failures.
    pub async fn execute(&self, request: &RequestSpec) -> ExecuteResult {
        // Validate request
        self.validate(request)?;

        // Execute via HTTP client
        let response = self.client.execute(request).await?;

        Ok(response)
    }

    /// Executes the request with cancellation support.
    ///
    /// # Arguments
    ///
    /// * `request` - The request to execute
    /// * `cancel` - Cancellation receiver for aborting the request
    ///
    /// # Returns
    ///
    /// The response, or an error if cancelled or failed.
    pub async fn execute_with_cancellation(
        &self,
        request: &RequestSpec,
        mut cancel: CancellationReceiver,
    ) -> ExecuteResult {
        // Validate request
        self.validate(request)?;

        // Race between execution and cancellation
        tokio::select! {
            result = self.client.execute(request) => {
                result.map_err(ExecuteRequestError::from)
            }
            () = cancel.cancelled() => {
                Err(ExecuteRequestError::HttpError(HttpClientError::Cancelled))
            }
        }
    }

    /// Validates the request before execution.
    fn validate(&self, request: &RequestSpec) -> Result<(), ExecuteRequestError> {
        // Check for empty URL
        if request.url.trim().is_empty() {
            return Err(ExecuteRequestError::EmptyUrl);
        }

        // Basic URL validation
        if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
            return Err(ExecuteRequestError::InvalidUrl(
                "URL must start with http:// or https://".to_string(),
            ));
        }

        Ok(())
    }
}

/// Extension trait for convenient `RequestState` conversion.
pub trait ExecuteResultExt {
    /// Converts the result to a `RequestState` for UI binding.
    fn to_request_state(self) -> RequestState;
}

impl ExecuteResultExt for ExecuteResult {
    fn to_request_state(self) -> RequestState {
        match self {
            Ok(response) => RequestState::success(response),
            Err(e) => e.to_request_state(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::future::Future;
    use std::pin::Pin;
    use std::time::Duration;

    /// Mock HTTP client for testing.
    struct MockHttpClient {
        response: Result<ResponseSpec, HttpClientError>,
    }

    impl MockHttpClient {
        fn success() -> Self {
            Self {
                response: Ok(ResponseSpec::new(
                    200u16,
                    HashMap::new(),
                    b"OK".to_vec(),
                    Duration::from_millis(50),
                )),
            }
        }

        fn error(err: HttpClientError) -> Self {
            Self { response: Err(err) }
        }
    }

    impl HttpClient for MockHttpClient {
        fn execute(
            &self,
            _request: &RequestSpec,
        ) -> Pin<Box<dyn Future<Output = Result<ResponseSpec, HttpClientError>> + Send + '_>>
        {
            let result = self.response.clone();
            Box::pin(async move { result })
        }
    }

    #[tokio::test]
    async fn test_execute_success() {
        let client = Arc::new(MockHttpClient::success());
        let use_case = ExecuteRequest::new(client);

        let request = RequestSpec::get("https://api.example.com/test");
        let result = use_case.execute(&request).await;

        assert!(result.is_ok());
        let response = result.expect("should be ok");
        assert_eq!(response.status, 200);
    }

    #[tokio::test]
    async fn test_execute_empty_url() {
        let client = Arc::new(MockHttpClient::success());
        let use_case = ExecuteRequest::new(client);

        let mut request = RequestSpec::new("Test");
        request.url = String::new();
        let result = use_case.execute(&request).await;

        assert!(matches!(result, Err(ExecuteRequestError::EmptyUrl)));
    }

    #[tokio::test]
    async fn test_execute_invalid_url() {
        let client = Arc::new(MockHttpClient::success());
        let use_case = ExecuteRequest::new(client);

        let mut request = RequestSpec::new("Test");
        request.url = "not-a-valid-url".to_string();
        let result = use_case.execute(&request).await;

        assert!(matches!(result, Err(ExecuteRequestError::InvalidUrl(_))));
    }

    #[tokio::test]
    async fn test_execute_http_error() {
        let client = Arc::new(MockHttpClient::error(HttpClientError::Timeout {
            timeout_ms: 5000,
        }));
        let use_case = ExecuteRequest::new(client);

        let request = RequestSpec::get("https://api.example.com/test");
        let result = use_case.execute(&request).await;

        assert!(matches!(
            result,
            Err(ExecuteRequestError::HttpError(
                HttpClientError::Timeout { .. }
            ))
        ));
    }

    #[tokio::test]
    async fn test_result_to_request_state() {
        let success_result: ExecuteResult = Ok(ResponseSpec::new(
            200u16,
            HashMap::new(),
            vec![],
            Duration::from_millis(100),
        ));
        let state = success_result.to_request_state();
        assert!(state.is_success());

        let error_result: ExecuteResult = Err(ExecuteRequestError::EmptyUrl);
        let state = error_result.to_request_state();
        assert!(state.is_error());
    }

    #[tokio::test]
    async fn test_error_to_request_state() {
        let error = ExecuteRequestError::EmptyUrl;
        let state = error.to_request_state();
        assert!(state.is_error());

        if let RequestState::Error { kind, message, .. } = state {
            assert_eq!(kind, RequestErrorKind::InvalidUrl);
            assert_eq!(message, "URL is required");
        }
    }
}
