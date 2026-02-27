# Build Stage
FROM rust:1.80-slim-bullseye AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy main to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -f target/release/deps/ig_engine*

# Copy source code
COPY src ./src

# Build for release
RUN cargo build --release

# Runtime Stage
FROM debian:bullseye-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/ig-engine /app/ig-engine

# Copy default config
COPY config /app/config

# Set environment variables
ENV RUST_LOG=info
ENV CONFIG_PATH=config/default.toml

# Expose API port
EXPOSE 9090

# Healthcheck
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:9090/api/health || exit 1

# Run the engine
ENTRYPOINT ["/app/ig-engine"]
