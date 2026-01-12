# syntax=docker/dockerfile:1.6
# =============================================================================
# Vortex Config Server - Distroless Dockerfile
# =============================================================================
# Final image: ~25MB (distroless + binary)
# Security: No shell, no package manager, minimal attack surface
# Uses gix (pure Rust Git) - no system git required

# -----------------------------------------------------------------------------
# Stage 1: Build with Debian for glibc compatibility
# -----------------------------------------------------------------------------
FROM rust:1.92-bookworm AS builder

WORKDIR /app

# Install minimal build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build with cache mounts for faster rebuilds
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked --package vortex-server \
    && cp /app/target/release/vortex-server /app/vortex-server

# Strip binary for smaller size
RUN strip /app/vortex-server

# -----------------------------------------------------------------------------
# Stage 2: Create directories with proper permissions
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS permissions

# Create directories that nonroot user (65532) can write to
RUN mkdir -p /home/nonroot/repos /home/nonroot/config \
    && chown -R 65532:65532 /home/nonroot

# -----------------------------------------------------------------------------
# Stage 3: Distroless runtime (no shell, no package manager)
# -----------------------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot

# Copy the binary
COPY --from=builder /app/vortex-server /usr/local/bin/vortex-server

# Copy pre-created directories with correct ownership
COPY --from=permissions --chown=65532:65532 /home/nonroot /home/nonroot

# Environment variables - use home directory for repos
ENV VORTEX_HOST=0.0.0.0 \
    VORTEX_PORT=8888 \
    RUST_LOG=info \
    GIT_LOCAL_PATH=/home/nonroot/repos

WORKDIR /home/nonroot

# Expose the default port
EXPOSE 8888

# Run as nonroot user (provided by distroless:nonroot)
ENTRYPOINT ["/usr/local/bin/vortex-server"]
