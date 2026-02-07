//! HTTP Client implementation using reqwest.
//!
//! This adapter implements the `HttpClient` port using the reqwest library.
//! It handles all HTTP communication for the application.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use reqwest::{Client, Method, Url};
use vortex_application::ports::{HttpClient, HttpClientError};
use vortex_domain::{
    request::{HttpMethod, RequestBody, RequestBodyKind, RequestSpec},
    response::ResponseSpec,
};

/// HTTP client implementation using reqwest.
///
/// This is the primary HTTP adapter for Vortex. It wraps `reqwest::Client`
/// and implements the `HttpClient` port from the application layer.
pub struct ReqwestHttpClient {
    client: Client,
}

impl ReqwestHttpClient {
    /// Creates a new HTTP client with default settings.
    ///
    /// Default configuration:
    /// - Connection timeout: 30 seconds
    /// - Follow redirects: up to 10
    /// - TLS verification: enabled
    /// - User-Agent: "Vortex/0.1.0"
    ///
    /// # Errors
    ///
    /// Returns an error if the client cannot be created.
    pub fn new() -> Result<Self, HttpClientError> {
        let client = Client::builder()
            .user_agent("Vortex/0.1.0")
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| HttpClientError::Other(e.to_string()))?;

        Ok(Self { client })
    }

    /// Creates a new HTTP client with a custom reqwest client.
    #[must_use]
    pub const fn with_client(client: Client) -> Self {
        Self { client }
    }

    /// Converts domain `HttpMethod` to reqwest `Method`.
    const fn to_reqwest_method(method: HttpMethod) -> Method {
        match method {
            HttpMethod::Get => Method::GET,
            HttpMethod::Post => Method::POST,
            HttpMethod::Put => Method::PUT,
            HttpMethod::Patch => Method::PATCH,
            HttpMethod::Delete => Method::DELETE,
            HttpMethod::Head => Method::HEAD,
            HttpMethod::Options => Method::OPTIONS,
        }
    }

    /// Builds the request body from domain `RequestBody`.
    fn build_body(
        builder: reqwest::RequestBuilder,
        body: &RequestBody,
    ) -> Result<reqwest::RequestBuilder, HttpClientError> {
        match &body.kind {
            RequestBodyKind::None => Ok(builder),

            RequestBodyKind::Raw { .. } => {
                // Check if content type is JSON and validate
                if body
                    .content_type()
                    .is_some_and(|ct| ct.contains("application/json"))
                    && !body.content.is_empty()
                {
                    // Validate JSON syntax
                    let _: serde_json::Value = serde_json::from_str(&body.content)
                        .map_err(|e| HttpClientError::InvalidBody(format!("Invalid JSON: {e}")))?;
                }
                Ok(builder.body(body.content.clone()))
            }

            RequestBodyKind::FormUrlEncoded => {
                // Send as body with appropriate content type
                // Content-Type header will be set separately
                Ok(builder.body(body.content.clone()))
            }

            RequestBodyKind::FormData => Err(HttpClientError::Other(
                "Multipart form data not yet implemented".to_string(),
            )),
        }
    }

    /// Maps reqwest errors to domain `HttpClientError`.
    fn map_error(error: reqwest::Error, timeout_ms: u64) -> HttpClientError {
        if error.is_timeout() {
            return HttpClientError::Timeout { timeout_ms };
        }

        if error.is_connect() {
            let message = error.to_string();
            if message.to_lowercase().contains("dns") || message.to_lowercase().contains("resolve")
            {
                return HttpClientError::DnsError {
                    host: error
                        .url()
                        .map(|u| u.host_str().unwrap_or("unknown").to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    message,
                };
            }
            if message.to_lowercase().contains("refused") {
                return HttpClientError::ConnectionRefused {
                    host: error
                        .url()
                        .map(|u| u.host_str().unwrap_or("unknown").to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    port: error.url().and_then(|u| u.port()).unwrap_or(80),
                };
            }
            return HttpClientError::ConnectionFailed(message);
        }

        if error.is_redirect() {
            return HttpClientError::TooManyRedirects { max: 10 };
        }

        HttpClientError::Other(error.to_string())
    }
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default HTTP client")
    }
}

impl HttpClient for ReqwestHttpClient {
    fn execute(
        &self,
        request: &RequestSpec,
    ) -> Pin<Box<dyn Future<Output = Result<ResponseSpec, HttpClientError>> + Send + '_>> {
        // Clone what we need to move into the async block
        let method = request.method;
        let url = request.full_url();
        let headers: Vec<_> = request.enabled_headers().cloned().collect();
        let body = request.body.clone();
        let timeout_ms = request.timeout_ms;

        Box::pin(async move {
            // Parse URL
            let parsed_url =
                Url::parse(&url).map_err(|e| HttpClientError::InvalidUrl(format!("{e}: {url}")))?;

            // Start timing
            let start = Instant::now();

            // Build request
            let mut builder = self
                .client
                .request(Self::to_reqwest_method(method), parsed_url)
                .timeout(Duration::from_millis(timeout_ms));

            // Add headers
            for header in &headers {
                builder = builder.header(&header.name, &header.value);
            }

            // Add Content-Type if body has one and not already set
            if let Some(content_type) = body.content_type() {
                let has_content_type = headers
                    .iter()
                    .any(|h| h.name.eq_ignore_ascii_case("content-type"));
                if !has_content_type {
                    builder = builder.header("Content-Type", content_type);
                }
            }

            // Add body
            builder = Self::build_body(builder, &body)?;

            // Execute request
            let response = builder
                .send()
                .await
                .map_err(|e| Self::map_error(e, timeout_ms))?;

            // Calculate duration
            let duration = start.elapsed();

            // Extract response data
            let status = response.status().as_u16();

            // Collect headers
            let response_headers: HashMap<String, String> = response
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
                .collect();

            // Read body
            let body_bytes = response
                .bytes()
                .await
                .map_err(|e| HttpClientError::Other(format!("Failed to read body: {e}")))?
                .to_vec();

            Ok(ResponseSpec::new(
                status,
                response_headers,
                body_bytes,
                duration,
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_reqwest_method() {
        assert_eq!(
            ReqwestHttpClient::to_reqwest_method(HttpMethod::Get),
            Method::GET
        );
        assert_eq!(
            ReqwestHttpClient::to_reqwest_method(HttpMethod::Post),
            Method::POST
        );
        assert_eq!(
            ReqwestHttpClient::to_reqwest_method(HttpMethod::Put),
            Method::PUT
        );
        assert_eq!(
            ReqwestHttpClient::to_reqwest_method(HttpMethod::Delete),
            Method::DELETE
        );
    }

    #[test]
    fn test_client_creation() {
        let client = ReqwestHttpClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_invalid_json_body() {
        let body = RequestBody::json("{invalid json}");
        let client = Client::new();
        let builder = client.post("https://example.com");
        let result = ReqwestHttpClient::build_body(builder, &body);
        assert!(matches!(result, Err(HttpClientError::InvalidBody(_))));
    }

    #[test]
    fn test_valid_json_body() {
        let body = RequestBody::json(r#"{"key": "value"}"#);
        let client = Client::new();
        let builder = client.post("https://example.com");
        let result = ReqwestHttpClient::build_body(builder, &body);
        assert!(result.is_ok());
    }
}
