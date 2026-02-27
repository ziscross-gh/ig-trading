# IG Trading System — Multi-stage Docker build
# Builds both Rust engine and Next.js dashboard

# ============================================
# Stage 1: Rust Engine Builder
# ============================================
FROM rust:1.82-slim AS rust-builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy engine source
COPY ig-engine/ ./

# Create dummy main to cache dependencies
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -f target/release/deps/ig_engine*

# Copy actual source and build for release
COPY ig-engine/src ./src
RUN cargo build --release

# ============================================
# Stage 2: Next.js Frontend Builder
# ============================================
FROM node:20-slim AS frontend-builder

WORKDIR /app

# Copy package files
COPY package.json package-lock.json* bun.lock* ./

# Install dependencies (try npm first, fallback to bun)
RUN if [ -f bun.lock ]; then \
      npm install -g bun && bun install --frozen-lockfile; \
    else \
      npm ci; \
    fi

# Copy source
COPY . .

# Build Next.js
ENV NEXT_TELEMETRY_DISABLED=1
RUN npm run build || (bun run build 2>/dev/null || true)

# ============================================
# Stage 3: Runtime (Rust + Node)
# ============================================
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    bash \
    && rm -rf /var/lib/apt/lists/*

# Copy Rust binary from builder
COPY --from=rust-builder /app/target/release/ig-engine /app/ig-engine
COPY ig-engine/config /app/engine-config

# Copy Next.js output from builder
COPY --from=frontend-builder /app/.next/standalone /app/dashboard
COPY --from=frontend-builder /app/.next/static /app/dashboard/.next/static
COPY --from=frontend-builder /app/public /app/dashboard/public

# Set environment variables
ENV RUST_LOG=info
ENV CONFIG_PATH=/app/engine-config/default.toml
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

# Expose both ports
EXPOSE 3000 9090

# Create entrypoint script — use printf so \n becomes real newlines
RUN printf '#!/bin/bash\nset -e\n\necho "Starting IG Trading System..."\n\n# Start Rust engine in background\necho "Starting engine on port 9090..."\n/app/ig-engine &\nENGINE_PID=$!\n\n# Poll /api/health until engine is ready (up to 30s)\necho "Waiting for engine to be ready..."\nfor i in $(seq 1 30); do\n  curl -sf http://localhost:9090/api/health > /dev/null 2>&1 && break\n  sleep 1\ndone\n\n# Start Next.js dashboard\necho "Starting dashboard on port 3000..."\ncd /app/dashboard && node server.js &\nDASHBOARD_PID=$!\n\n# Forward SIGTERM/SIGINT to both child processes\ntrap "kill $ENGINE_PID $DASHBOARD_PID 2>/dev/null; wait" SIGTERM SIGINT\n\n# Block until either process exits, then shut down both\nwait -n $ENGINE_PID $DASHBOARD_PID\nEXIT_CODE=$?\nkill $ENGINE_PID $DASHBOARD_PID 2>/dev/null\nwait\nexit $EXIT_CODE\n' > /app/entrypoint.sh && chmod +x /app/entrypoint.sh

CMD ["/app/entrypoint.sh"]
