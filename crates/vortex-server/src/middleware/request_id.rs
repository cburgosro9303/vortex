//! Middleware que genera o propaga X-Request-Id.

use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, Request, Response},
};
use std::task::{Context, Poll};
use tower::{Layer, Service};
use uuid::Uuid;

/// Header name for request ID.
pub static REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Layer that adds request ID to requests and responses.
#[derive(Clone, Default)]
pub struct RequestIdLayer;

impl<S> Layer<S> for RequestIdLayer {
    type Service = RequestIdMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestIdMiddleware { inner }
    }
}

/// Middleware that ensures every request has a unique ID.
#[derive(Clone)]
pub struct RequestIdMiddleware<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for RequestIdMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        // Get existing request ID or generate new one
        let request_id = request
            .headers()
            .get(&REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .unwrap_or_else(|| Uuid::now_v7().to_string());

        // Add request ID to request headers (for handlers to access)
        if let Ok(value) = HeaderValue::from_str(&request_id) {
            request
                .headers_mut()
                .insert(REQUEST_ID_HEADER.clone(), value);
        }

        // Store request ID for response
        let request_id_for_response = request_id;

        let mut inner = self.inner.clone();

        Box::pin(async move {
            let mut response = inner.call(request).await?;

            // Add request ID to response headers
            if let Ok(value) = HeaderValue::from_str(&request_id_for_response) {
                response
                    .headers_mut()
                    .insert(REQUEST_ID_HEADER.clone(), value);
            }

            Ok(response)
        })
    }
}

// Unit tests are in tests/middleware_test.rs to avoid complex type bounds
// with tower::service_fn and async functions.
