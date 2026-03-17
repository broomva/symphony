# Stage 1: Build
FROM rust:1.85-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src/ src/
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates git curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/symphony /usr/local/bin/symphony
WORKDIR /workspace
ENTRYPOINT ["symphony"]
CMD ["WORKFLOW.md"]
