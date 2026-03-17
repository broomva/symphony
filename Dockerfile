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
    ca-certificates git curl gnupg sudo \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
       | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
       > /etc/apt/sources.list.d/github-cli.list \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get update && apt-get install -y --no-install-recommends gh nodejs \
    && rm -rf /var/lib/apt/lists/*

# Install Claude Code CLI globally
RUN npm install -g @anthropic-ai/claude-code

# Create non-root user (Claude Code refuses --dangerously-skip-permissions as root)
RUN useradd -m -s /bin/bash symphony \
    && mkdir -p /app/workspaces \
    && chown -R symphony:symphony /app

# Copy Symphony binary and startup script
COPY --from=builder /app/target/release/symphony /usr/local/bin/symphony
COPY --chown=symphony:symphony WORKFLOW.md /app/WORKFLOW.md
COPY --chown=symphony:symphony start.sh /app/start.sh

WORKDIR /app
USER symphony

ENV SYMPHONY_BIND=0.0.0.0
EXPOSE 8080

# start.sh fetches WORKFLOW.md from control plane if SYMPHONY_CLOUD_CONFIG_URL is set
CMD ["/app/start.sh"]
