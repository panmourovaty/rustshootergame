# syntax=docker/dockerfile:1

# ── Build stage ───────────────────────────────────────────────────────────────
FROM rust:alpine AS builder

# musl-dev: C headers for musl libc (ring crate needs them)
# pkgconfig: required by some build scripts
RUN apk add --no-cache \
    musl-dev \
    pkgconfig

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN cargo build \
    --release \
    --bin server \
    --no-default-features \
    --features server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM alpine:3

# ca-certificates: needed for outbound TLS (WebTransport certificate validation)
RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/server /usr/local/bin/server

# UDP port for native clients
EXPOSE 7777/udp
# TCP port for WebTransport (browser clients)
EXPOSE 7778/tcp

ENTRYPOINT ["/usr/local/bin/server"]
