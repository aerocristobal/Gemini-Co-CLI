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

# Install Google Generative AI SDK
RUN pip3 install --break-system-packages google-generativeai google-auth-oauthlib

# Create app directory
WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/gemini-co-cli .

# Copy static files
COPY --from=builder /app/static ./static

# Copy and install Gemini CLI wrapper
COPY scripts/gemini-cli.py /usr/local/bin/gemini-cli.py
RUN chmod +x /usr/local/bin/gemini-cli.py && \
    ln -s /usr/local/bin/gemini-cli.py /usr/local/bin/gemini

# Expose port
EXPOSE 3000

# Set environment variables
ENV RUST_LOG=info
ENV PATH="/usr/local/bin:${PATH}"

# Note: Users must authenticate Gemini CLI before running:
# Option 1 - API Key (simpler):
#   docker run -e GOOGLE_API_KEY=your_key <image> gemini auth login
# Option 2 - OAuth (more secure):
#   Place credentials.json in ~/.config/gemini-co-cli/ and run:
#   docker run -it <image> gemini auth login

CMD ["./gemini-co-cli"]
