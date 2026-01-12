//! Vortex Config Server binary.

use std::net::SocketAddr;

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get configuration from environment
    let host = std::env::var("VORTEX_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("VORTEX_PORT")
        .unwrap_or_else(|_| "8888".to_string())
        .parse::<u16>()
        .expect("VORTEX_PORT must be a valid port number");

    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .expect("Invalid address");

    tracing::info!(
        "Starting Vortex Config Server v{}",
        env!("CARGO_PKG_VERSION")
    );

    vortex_server::run_server(addr).await?;

    Ok(())
}
