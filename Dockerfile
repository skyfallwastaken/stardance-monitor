# syntax=docker/dockerfile:1
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
 && rm -rf /var/lib/apt/lists/*

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked --bin stardance_monitor && \
    cp /app/target/release/stardance_monitor /app/stardance_monitor

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

RUN useradd -u 1000 -m appuser
USER appuser
WORKDIR /app

COPY --from=builder /app/stardance_monitor /app/stardance_monitor
COPY --chown=appuser:appuser scripts/run-every-5min.sh /app/run-every-5min.sh
RUN chmod +x /app/run-every-5min.sh

EXPOSE 8080

ENTRYPOINT ["/app/run-every-5min.sh"]
