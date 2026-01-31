# Build stage
FROM rust:latest AS builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy manifests (including Cargo.lock for reproducible builds)
COPY api/Cargo.toml api/Cargo.lock ./

# Copy migrations for sqlx::migrate! macro at compile time
COPY migrations /migrations

# Create dummy main to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Copy actual source
COPY api/src ./src

# Build the application
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/raptor-api /app/raptor-api

# Copy migrations
COPY migrations /app/migrations

# Copy email templates
COPY api/templates /app/templates

EXPOSE 3000

CMD ["/app/raptor-api"]
