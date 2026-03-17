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
    ca-certificates git curl gh \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/symphony /usr/local/bin/symphony
COPY WORKFLOW.md /app/WORKFLOW.md
WORKDIR /app

# Railway provides PORT; Symphony reads it for server binding
ENV SYMPHONY_BIND=0.0.0.0
EXPOSE 8080

ENTRYPOINT ["symphony"]
CMD ["start", "--port", "8080", "WORKFLOW.md"]
