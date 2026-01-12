//! HTTP metrics middleware.

use axum::{body::Body, extract::MatchedPath, http::Request, middleware::Next, response::Response};
use metrics::{counter, histogram};
use std::time::Instant;

/// Middleware que registra metricas HTTP para cada request.
pub async fn http_metrics_middleware(
    matched_path: Option<MatchedPath>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = request.method().to_string();
    let path = matched_path
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());

    let response = next.run(request).await;

    let status = response.status().as_u16().to_string();
    let duration = start.elapsed();

    // Registrar metricas
    counter!(
        "vortex_http_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status
    )
    .increment(1);

    histogram!(
        "vortex_http_request_duration_seconds",
        "method" => method,
        "path" => path
    )
    .record(duration.as_secs_f64());

    response
}

/// Registra las metricas HTTP
pub fn register_http_metrics() {
    metrics::describe_counter!(
        "vortex_http_requests_total",
        "Total number of HTTP requests"
    );
    metrics::describe_histogram!(
        "vortex_http_request_duration_seconds",
        "HTTP request duration in seconds"
    );
}
