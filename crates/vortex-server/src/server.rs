use std::net::SocketAddr;

use axum::{Router, routing::get};
use tower::ServiceBuilder;

use crate::handlers::{
    config::{get_config, get_config_with_label},
    health::health_check,
};
use crate::middleware::{LoggingLayer, RequestIdLayer};
use crate::state::AppState;

/// Creates a router with the given application state.
pub fn create_router_with_state(state: AppState) -> Router {
    let middleware = ServiceBuilder::new()
        .layer(RequestIdLayer)
        .layer(LoggingLayer);

    Router::new()
        .route("/health", get(health_check))
        .route("/:app/:profile/:label", get(get_config_with_label))
        .route("/:app/:profile", get(get_config))
        .layer(middleware)
        .with_state(state)
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

/// Runs the server with the given state.
pub async fn run_server_with_state(
    addr: SocketAddr,
    state: AppState,
) -> Result<(), std::io::Error> {
    let app = create_router_with_state(state);

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
