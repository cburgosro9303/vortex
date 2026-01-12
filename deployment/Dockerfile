# syntax=docker/dockerfile:1.6
# =============================================================================
# Vortex Config Server - Optimized Alpine Dockerfile
# =============================================================================
# Final image: ~37MB (Alpine + git + optimized binary)
# Security: Non-root user, minimal attack surface
# Optimizations: LTO, opt-level=z, strip, codegen-units=1, panic=abort

# -----------------------------------------------------------------------------
# Stage 1: Build with musl for Alpine compatibility
# -----------------------------------------------------------------------------
FROM rust:1.92-alpine AS builder

WORKDIR /app

# Install minimal build dependencies
RUN apk add --no-cache musl-dev

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build with cache mounts for faster rebuilds
# Binary is automatically stripped via Cargo profile (strip = true)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked --package vortex-server \
    && cp /app/target/release/vortex-server /app/vortex-server

# -----------------------------------------------------------------------------
# Stage 2: Minimal Alpine runtime
# -----------------------------------------------------------------------------
FROM alpine:3.21 AS runtime

# Install only essential runtime dependencies
# - ca-certificates: for HTTPS Git clones
# - git: required for repository operations
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
COPY --from=builder /app/vortex-server /usr/local/bin/vortex-server

# Switch to non-root user
USER vortex

WORKDIR /home/vortex

# Environment variables
ENV VORTEX_HOST=0.0.0.0 \
    VORTEX_PORT=8888 \
    RUST_LOG=info

# Expose the default port
EXPOSE 8888

# Health check using wget (included in busybox)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -q --spider http://127.0.0.1:8888/health || exit 1

# Run the server
ENTRYPOINT ["vortex-server"]
