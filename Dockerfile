# ── Stage 1: Build ─────────────────────────────────────────────────────────────
# rust:alpine uses the musl toolchain, producing a fully static binary that
# runs on any Alpine (or other musl-based) Linux container with zero extra deps.
FROM rust:alpine AS builder

# musl-dev  — C runtime headers required by some Rust crates (ring, etc.)
# pkgconfig — lets build scripts locate system libraries
RUN apk add --no-cache musl-dev pkgconfig

WORKDIR /build

# Copy manifest files first so cargo's dependency fetch layer is cached
# independently of source changes.
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

# Build the headless server binary.
# --no-default-features strips the client/WASM networking stack.
# --features server       enables the server-only networking stack.
# The server uses MinimalPlugins (no renderer, no window), so no graphics
# libraries are needed at build or run time.
RUN cargo build \
    --release \
    --bin server \
    --no-default-features \
    --features server

# ── Stage 2: Runtime ───────────────────────────────────────────────────────────
FROM alpine:3.21

# ca-certificates — required for outbound TLS (WebTransport self-signed cert
#                   chain validation, crates.io, etc.)
# libgcc          — unwinding support (needed by Rust panic handlers)
RUN apk add --no-cache ca-certificates libgcc

COPY --from=builder /build/target/release/server /usr/local/bin/server

# UDP port for native clients (default 7777) and
# WebTransport port for browser/WASM clients (default 7778).
EXPOSE 7777/udp
EXPOSE 7778/udp

ENTRYPOINT ["/usr/local/bin/server"]
