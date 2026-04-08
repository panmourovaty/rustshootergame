# RustShooter

A Counter-Strike-inspired FPS built with Bevy + lightyear.

---

## Table of Contents

1. [Quick Start — LAN Play](#1-quick-start--lan-play)
2. [Choosing the Right Binary](#2-choosing-the-right-binary)
3. [Running the Server](#3-running-the-server)
4. [Running the Native Client](#4-running-the-native-client)
5. [Environment Variable: RSG_SERVER_ADDR](#5-environment-variable-rsg_server_addr)
6. [Browser / WASM Client](#6-browser--wasm-client)
7. [Building from Source](#7-building-from-source)

---

## 1. Quick Start — LAN Play

This is the minimum to get two people playing on the same network.

**On the machine that will host:**

```
./server
```

The server prints its IP and ports on startup:

```
===========================================
 RustShooter dedicated server is running
===========================================
 UDP  (native clients)  : 0.0.0.0:7777
 WebTransport (browser) : 0.0.0.0:7778
===========================================
```

**On each player's machine:**

```
./client
```

At the connect screen enter:
- **Username** — anything you like
- **Server Address** — the LAN IP of the host machine (e.g. `192.168.1.10`)

Click **CONNECT** or press **Enter**.

> Port `7777` is assumed when you don't type one. You can also type `192.168.1.10:7777` explicitly.

---

## 2. Choosing the Right Binary

Download from the [latest GitHub Release](../../releases/latest).

### Client

| File | OS | When to use |
|------|----|-------------|
| `client-windows-x86_64.exe` | Windows | Any modern Windows PC (Haswell CPU or newer) |
| `client-linux-amd64` | Linux x86-64 | Safe fallback — works on any x86-64 CPU |
| `client-linux-amd64-v3` | Linux x86-64 | Faster — requires Haswell / Zen 1+ (2013+) |
| `client-linux-arm64` | Linux ARM64 | Safe fallback — any AArch64 board/device |
| `client-linux-arm64-sve` | Linux ARM64 | Faster — requires Graviton 3 / Neoverse V1+ |
| `client-macos-arm64` | macOS Apple Silicon | M1/M2/M3/M4 Macs |
| `client-web.zip` | Browser | See [§6 Browser / WASM Client](#6-browser--wasm-client) |

**Rule of thumb:** download the `-v3` or `-sve` variant first. If the binary immediately crashes with `Illegal instruction`, fall back to the plain baseline binary.

### Server

| File | OS | When to use |
|------|----|-------------|
| `server-linux-amd64` | Linux x86-64 | Safe fallback — any x86-64 CPU |
| `server-linux-amd64-v3` | Linux x86-64 | Faster — requires Haswell / Zen 1+ |
| `server-linux-arm64` | Linux ARM64 | Safe fallback — any AArch64 |
| `server-linux-arm64-sve` | Linux ARM64 | Faster — requires Graviton 3 / Neoverse V1+ |

The server has no GUI. Run it headless on any Linux machine or VPS.

---

## 3. Running the Server

### Basic usage

```bash
chmod +x server-linux-amd64    # first time only
./server-linux-amd64
```

### Options

```
./server --help

Options:
  --port <PORT>         UDP port for native clients     [default: 7777]
  --web-port <PORT>     WebTransport port for browser   [default: 7778]
```

### Examples

```bash
# Default ports (UDP 7777, WebTransport 7778)
./server

# Custom ports
./server --port 9000 --web-port 9001
```

### Firewall

Open both ports (TCP+UDP) on the host's firewall:

```bash
# UFW (Ubuntu / Debian)
sudo ufw allow 7777
sudo ufw allow 7778

# firewalld (Fedora / RHEL)
sudo firewall-cmd --add-port=7777/udp --add-port=7778/tcp --permanent
sudo firewall-cmd --reload
```

For internet play, also forward both ports in your router if the server is behind NAT.

---

## 4. Running the Native Client

### Normal interactive mode

```bash
chmod +x client-linux-amd64    # first time only
./client-linux-amd64
```

The connect screen appears. Enter your username and the server address, then click **CONNECT** or press **Enter**.

- You can type an IP address (`192.168.1.10`), a domain name (`play.example.com`), or include a port (`192.168.1.10:9000`).
- If you omit the port, `:7777` is used automatically.
- The connection attempt has a **15-second timeout**. If it fails, you are returned to the connect screen with an error message.
- You can cancel mid-attempt with the **Cancel** button.

### Pre-set server address (kiosk / event mode)

Set `RSG_SERVER_ADDR` before launching — see [§5](#5-environment-variable-rsg_server_addr).

---

## 5. Environment Variable: RSG_SERVER_ADDR

When `RSG_SERVER_ADDR` is set, the **Server Address field is hidden** from the connect screen. Players only need to enter a username and hit **CONNECT** — useful for LAN events, managed deployments, or bundled executables.

```bash
# Linux / macOS
export RSG_SERVER_ADDR=192.168.1.10
./client

# Windows (Command Prompt)
set RSG_SERVER_ADDR=192.168.1.10
client.exe

# Windows (PowerShell)
$env:RSG_SERVER_ADDR = "192.168.1.10"
./client.exe
```

Accepted formats:

| Value | Resolved as |
|-------|-------------|
| `192.168.1.10` | `192.168.1.10:7777` |
| `192.168.1.10:9000` | `192.168.1.10:9000` |
| `play.example.com` | DNS lookup → `:7777` |
| `play.example.com:9000` | DNS lookup → `:9000` |

---

## 6. Browser / WASM Client

The WASM client connects over **WebTransport** (HTTPS/HTTP3), not plain UDP. This requires a small amount of extra setup because browsers require a TLS certificate to be trusted before they will open a WebTransport session.

### How it works

1. The server generates a **self-signed TLS certificate** at startup and prints its SHA-256 fingerprint:

   ```
   WebTransport listener on 0.0.0.0:7778 | cert digest: 4a9f3c...
   → To build the WASM client: RSG_CERT_DIGEST=4a9f3c... cargo build ...
   ```

2. The WASM binary must be compiled with that fingerprint **baked in** via the `RSG_CERT_DIGEST` environment variable. The compiled WASM then presents the fingerprint to the browser, which allows the self-signed cert.

3. Each time the server restarts it generates a **new** certificate. The WASM build must be recompiled (or the server must keep its cert on disk between restarts — not yet implemented).

### Using the pre-built WASM from GitHub Releases

The `client-web.zip` in every release is also deployed automatically to **Cloudflare Pages**. However, that build's `RSG_CERT_DIGEST` was baked at CI time against a cert that no longer exists once the server restarts.

For a persistent server you should build the WASM yourself (see [§7.3](#73-wasm-client)).

### Playing in the browser

1. Extract `client-web.zip` and serve it with any static web server that supports HTTPS and HTTP/3 (required by WebTransport).
2. Open the URL in Chrome or Edge (Firefox does not yet support WebTransport over self-signed certs).
3. Enter your username and click **CONNECT**. The server address field shows the address baked into the build.

---

## 7. Building from Source

### Prerequisites

- Rust stable toolchain (`rustup update stable`)
- For Linux clients: system packages
  ```bash
  sudo apt-get install -y libasound2-dev libudev-dev libxkbcommon-dev
  ```

### 7.1 Native client

```bash
cargo build --release --bin client
# Output: target/release/client
```

With x86-64-v3 optimisations (requires Haswell / Zen 1+ CPU):

```bash
RUSTFLAGS="-C target-cpu=x86-64-v3" cargo build --release --bin client
```

### 7.2 Server

```bash
cargo build --release --bin server
# Output: target/release/server
```

### 7.3 WASM client

Step 1 — start the server and copy the cert digest from its startup output:

```
WebTransport listener on 0.0.0.0:7778 | cert digest: 4a9f3cABCD...
```

Step 2 — compile the WASM client with that digest:

```bash
RSG_CERT_DIGEST=4a9f3cABCD... \
cargo build \
  --target wasm32-unknown-unknown \
  --no-default-features \
  --features web \
  --profile wasm-release \
  --bin client
```

Step 3 — run `wasm-bindgen` and optimise with `wasm-opt`:

```bash
wasm-bindgen \
  --out-dir dist/ \
  --target web \
  target/wasm32-unknown-unknown/wasm-release/client.wasm

# Shrink and optimise the output (requires binaryen — see below)
wasm-opt -Oz --strip-debug --vacuum \
  dist/client_bg.wasm \
  -o dist/client_bg.wasm
```

Install `wasm-opt` via your package manager if you don't have it:

```bash
# Ubuntu / Debian
sudo apt-get install binaryen

# macOS
brew install binaryen

# Windows (winget)
winget install WebAssembly.Binaryen
```

Step 4 — serve `dist/` over HTTPS. Any local dev server with TLS works (e.g. `caddy file-server --listen :8443 --root dist/`).

> **Note:** Leave `RSG_CERT_DIGEST` unset (or empty) if your server uses a proper CA-signed certificate — the browser will trust it automatically.

### 7.4 Cargo features

| Feature | What it enables |
|---------|-----------------|
| `networking` (default) | UDP + WebTransport — native client + full server |
| `web` | WebTransport only — WASM client (no UDP, no server code) |
