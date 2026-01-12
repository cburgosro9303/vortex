//! Vortex Config Server binary.

use std::net::SocketAddr;
use std::path::PathBuf;

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use vortex_git::{GitBackend, GitBackendConfig};
use vortex_server::{AppState, run_server_with_state};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get server configuration from environment
    let host = std::env::var("VORTEX_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("VORTEX_PORT")
        .unwrap_or_else(|_| "8888".to_string())
        .parse::<u16>()
        .expect("VORTEX_PORT must be a valid port number");

    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .expect("Invalid address");

    // Get Git configuration from environment
    let git_uri = std::env::var("GIT_URI").expect("GIT_URI environment variable is required");
    let git_local_path =
        std::env::var("GIT_LOCAL_PATH").unwrap_or_else(|_| "/var/lib/vortex/repos".to_string());
    let git_default_label =
        std::env::var("GIT_DEFAULT_LABEL").unwrap_or_else(|_| "main".to_string());

    // Build Git backend configuration
    let mut config_builder = GitBackendConfig::builder()
        .uri(&git_uri)
        .local_path(PathBuf::from(&git_local_path))
        .default_label(&git_default_label);

    // Add search paths if configured
    if let Ok(search_paths) = std::env::var("GIT_SEARCH_PATHS") {
        let paths: Vec<String> = search_paths
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        config_builder = config_builder.search_paths(paths);
    }

    // Add authentication if configured
    if let (Ok(username), Ok(password)) =
        (std::env::var("GIT_USERNAME"), std::env::var("GIT_PASSWORD"))
    {
        config_builder = config_builder.basic_auth(username, password);
    }

    let git_config = config_builder
        .build()
        .expect("Failed to build Git configuration");

    tracing::info!(
        "Starting Vortex Config Server v{}",
        env!("CARGO_PKG_VERSION")
    );
    tracing::info!("Git repository: {}", git_uri);
    tracing::info!("Local path: {}", git_local_path);
    tracing::info!("Default label: {}", git_default_label);

    // Initialize Git backend (clones repository if needed)
    tracing::info!("Initializing Git backend...");
    let backend = GitBackend::new(git_config)
        .await
        .expect("Failed to initialize Git backend");

    tracing::info!("Git backend initialized successfully");

    // Create application state
    let state = AppState::from_git_backend(backend);

    // Run server
    run_server_with_state(addr, state).await?;

    Ok(())
}
