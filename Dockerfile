# Build stage
FROM rust:1.83-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
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

# Install runtime dependencies including Python for Gemini CLI
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    python3 \
    python3-pip \
    python3-venv \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Gemini CLI globally
RUN pip3 install --break-system-packages google-generativeai-cli

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

# Note: Users must authenticate Gemini CLI before running:
# docker run -it <image> gemini auth login
# Then run the application:
# CMD ["./gemini-co-cli"]

CMD ["./gemini-co-cli"]
