# syntax=docker/dockerfile:1

# ── Build stage ───────────────────────────────────────────────────────────────
FROM rust:alpine AS builder

# Build-time system dependencies required by transitive Bevy/wgpu deps.
# These are only needed in the builder; the final Alpine runtime image
# stays clean because the server uses MinimalPlugins (no display/audio)
# and LTO + --as-needed drops all unreferenced shared-library symbols.
#
#   musl-dev          C standard-library headers (ring, proc-macro crates)
#   pkgconfig         pkg-config tool used by several build scripts
#   wayland-dev       wayland-client.pc  (bevy_winit → winit → wayland-sys)
#   libxkbcommon-dev  xkbcommon.pc       (winit keyboard-layout handling)
#   eudev-dev         udev.pc            (bevy_gilrs gamepad support)
#   alsa-lib-dev      alsa.pc            (bevy_audio / cpal)
#   libx11-dev        x11.pc             (winit X11 backend)
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    wayland-dev \
    libxkbcommon-dev \
    eudev-dev \
    alsa-lib-dev \
    libx11-dev

WORKDIR /app

# GIT_HASH is injected by CI via --build-arg; defaults to "unknown" for
# local docker builds where the .git directory is absent (.dockerignore).
ARG GIT_HASH=unknown
ENV GIT_HASH=$GIT_HASH

COPY Cargo.toml Cargo.lock build.rs ./
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
