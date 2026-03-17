# Stage 1: Build Symphony
FROM rust:1.88-slim AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Reduce memory usage for Railway builds
ENV CARGO_INCREMENTAL=0
ENV CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
ENV CARGO_PROFILE_RELEASE_LTO=thin

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src/ src/
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

# System deps + git + gh CLI + Node.js (for Claude Code CLI)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates git curl gnupg \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
       | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
       > /etc/apt/sources.list.d/github-cli.list \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get update && apt-get install -y --no-install-recommends gh nodejs \
    && rm -rf /var/lib/apt/lists/*

# Install Claude Code CLI globally
RUN npm install -g @anthropic-ai/claude-code

# Copy Symphony binary
COPY --from=builder /app/target/release/symphony /usr/local/bin/symphony
COPY WORKFLOW.md /app/WORKFLOW.md
WORKDIR /app

# Claude Code uses ANTHROPIC_API_KEY for headless auth (no interactive login needed)
# Set via Railway env vars: ANTHROPIC_API_KEY, LINEAR_API_KEY, etc.
ENV SYMPHONY_BIND=0.0.0.0
EXPOSE 8080

# Use shell form so $PORT is expanded at runtime
CMD symphony start --port ${PORT:-8080} WORKFLOW.md
