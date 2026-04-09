# ── Build stage ───────────────────────────────────────────────────────────────
FROM rust:latest AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libwayland-dev \
    libxkbcommon-dev \
    libudev-dev \
    libasound2-dev \
    libx11-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

ARG GIT_HASH=unknown
ENV GIT_HASH=$GIT_HASH

COPY Cargo.toml Cargo.lock ./
COPY build.rs ./ 
COPY src/ src/

RUN cargo build \
    --release \
    --bin server \
    --no-default-features \
    --features server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:stable-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libx11-6 \
    libasound2 \
    libudev1 \
    libxkbcommon0 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/server /usr/local/bin/server

EXPOSE 7777/udp
EXPOSE 7778/tcp

ENTRYPOINT ["/usr/local/bin/server"]
