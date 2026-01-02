# Build stage - use bookworm variant to match runtime glibc version
FROM rust:slim-bookworm AS builder

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

# Install runtime dependencies including Node.js for official Gemini CLI
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    gnupg \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js 20.x (required for Gemini CLI)
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
    apt-get install -y nodejs && \
    rm -rf /var/lib/apt/lists/*

# Install official Gemini CLI globally
RUN npm install -g @google/gemini-cli

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

# Note: Authentication with Gemini CLI
# Option 1 - API Key (simpler):
#   Set GEMINI_API_KEY environment variable
#   docker run -e GEMINI_API_KEY=your_key <image>
#
# Option 2 - Google OAuth (recommended):
#   Run the container interactively first to authenticate:
#   docker run -it <image> gemini
#   Then select "Login with Google" and follow browser prompts
#   Credentials will be cached in the volume for future sessions

CMD ["./gemini-co-cli"]
