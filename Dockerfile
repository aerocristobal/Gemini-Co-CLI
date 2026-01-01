# Build stage
FROM rust:1.83-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml ./

# Copy source code
COPY src ./src
COPY static ./static

# Build for release
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/gemini-co-cli .

# Copy static files
COPY --from=builder /app/static ./static

# Expose port
EXPOSE 3000

# Set environment variables
ENV RUST_LOG=info

# Run the application
CMD ["./gemini-co-cli"]
