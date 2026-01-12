# =============================================================================
# Vortex Config Server - Multi-stage Dockerfile
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Build
# -----------------------------------------------------------------------------
FROM rust:1.92-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build the application in release mode
RUN cargo build --release --package vortex-server

# -----------------------------------------------------------------------------
# Stage 2: Runtime
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
# - ca-certificates: for HTTPS connections
# - git: for git backend operations
# - libssl3: for TLS support
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    git \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && git config --global --add safe.directory '*'

# Create non-root user for security
RUN groupadd --gid 1000 vortex \
    && useradd --uid 1000 --gid vortex --shell /bin/bash --create-home vortex

# Create directories for configuration and repository storage
RUN mkdir -p /var/lib/vortex/repos /etc/vortex \
    && chown -R vortex:vortex /var/lib/vortex /etc/vortex

# Copy the binary from builder
COPY --from=builder /app/target/release/vortex-server /usr/local/bin/vortex-server

# Switch to non-root user
USER vortex

WORKDIR /home/vortex

# Environment variables
ENV VORTEX_HOST=0.0.0.0
ENV VORTEX_PORT=8888
ENV RUST_LOG=info

# Expose the default port
EXPOSE 8888

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8888/health || exit 1

# Run the server
ENTRYPOINT ["vortex-server"]
