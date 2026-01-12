use std::net::SocketAddr;

use axum::{
    Router, middleware,
    routing::{delete, get},
};
use metrics_exporter_prometheus::PrometheusHandle;
use tower::ServiceBuilder;

use crate::handlers::{
    config::{get_config, get_config_with_label},
    health::health_check,
    invalidate::{
        invalidate_all, invalidate_by_app, invalidate_by_app_profile,
        invalidate_by_app_profile_label,
    },
    metrics::metrics_handler,
};
use crate::middleware::{LoggingLayer, RequestIdLayer};
use crate::state::AppState;

/// Creates a router with the given application state and metrics handle.
pub fn create_router_with_state(state: AppState, prometheus_handle: PrometheusHandle) -> Router {
    let middleware_stack = ServiceBuilder::new()
        .layer(RequestIdLayer)
        .layer(LoggingLayer);

    // Router for metrics endpoint (different state)
    let metrics_router = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(prometheus_handle);

    // Main application router
    let app_router = Router::new()
        .route("/health", get(health_check))
        // Config routes
        .route("/:app/:profile/:label", get(get_config_with_label))
        .route("/:app/:profile", get(get_config))
        // Cache invalidation routes
        .route("/cache", delete(invalidate_all))
        .route("/cache/:app", delete(invalidate_by_app))
        .route("/cache/:app/:profile", delete(invalidate_by_app_profile))
        .route(
            "/cache/:app/:profile/:label",
            delete(invalidate_by_app_profile_label),
        )
        .with_state(state);

    // Merge routers and apply middleware
    Router::new()
        .merge(app_router)
        .merge(metrics_router)
        // HTTP metrics middleware
        .layer(middleware::from_fn(
            crate::metrics::http::http_metrics_middleware,
        ))
        .layer(middleware_stack)
}

/// Creates a router without state (for testing only - health endpoint).
pub fn create_router() -> Router {
    let middleware = ServiceBuilder::new()
        .layer(RequestIdLayer)
        .layer(LoggingLayer);

    Router::new()
        .route("/health", get(health_check))
        .layer(middleware)
}

/// Runs the server with the given state and metrics handle.
pub async fn run_server_with_state(
    addr: SocketAddr,
    state: AppState,
    prometheus_handle: PrometheusHandle,
) -> Result<(), std::io::Error> {
    let app = create_router_with_state(state, prometheus_handle);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
}

/// Runs the server without state (for backward compatibility - health only).
pub async fn run_server(addr: SocketAddr) -> Result<(), std::io::Error> {
    let app = create_router();

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received");
}
