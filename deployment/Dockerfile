# =============================================================================
# Vortex Config Server - Optimized Multi-stage Dockerfile
# =============================================================================
# Final image: ~39MB (Alpine + git + binary)
# Security: Non-root user, minimal attack surface, no unnecessary packages

# -----------------------------------------------------------------------------
# Stage 1: Build with musl for static linking
# -----------------------------------------------------------------------------
FROM rustlang/rust:nightly-alpine AS builder

WORKDIR /app

# Install build dependencies (minimal)
RUN apk add --no-cache musl-dev

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build the application in release mode
RUN cargo build --release --package vortex-server

# Strip the binary for smaller size
RUN strip /app/target/release/vortex-server

# -----------------------------------------------------------------------------
# Stage 2: Minimal runtime
# -----------------------------------------------------------------------------
FROM alpine:3.21 AS runtime

# Install only essential runtime dependencies
# - ca-certificates: for HTTPS Git clones
# - git: for repository operations (required by vortex-git)
# - libgcc: required for Rust panic unwinding
RUN apk add --no-cache \
    ca-certificates \
    git \
    libgcc \
    && git config --global --add safe.directory '*'

# Create non-root user for security
RUN addgroup -g 1000 vortex \
    && adduser -u 1000 -G vortex -s /bin/sh -D vortex

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

# Health check using wget (smaller than curl)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -q --spider http://localhost:8888/health || exit 1

# Run the server
ENTRYPOINT ["vortex-server"]
