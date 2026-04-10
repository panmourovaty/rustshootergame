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
8. [WebTransport Certificate Setup](#8-webtransport-certificate-setup)

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
 UDP  (native clients)  : [::]:7777
 WebTransport (browser) : [::]:7778
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

All settings are controlled by a TOML configuration file. Copy `example_server_config.toml` from the repository to `server_config.toml` in the same directory as the binary and edit it:

```bash
cp example_server_config.toml server_config.toml
# edit server_config.toml as needed, then:
./server
```

The server looks for `server_config.toml` in the current working directory. You can also point it at a different file by passing the path as the first argument:

```bash
./server /etc/rustshooter/config.toml
```

If neither a path argument nor `server_config.toml` is found, built-in defaults are used (UDP 7777, WebTransport 7778, kill limit 20, self-signed cert).

### Configuration reference

All fields are optional. Unset fields use the default shown.

| Field | Default | Description |
|-------|---------|-------------|
| `port` | `7777` | UDP port for native (desktop) clients |
| `web_port` | `7778` | WebTransport (HTTP/3) port for browser clients |
| `kill_limit` | `20` | Kills required to end the round |
| `map_url` | _(none)_ | HTTPS URL of a `.tar.zst` map archive |
| `cert` | _(none)_ | Path to TLS certificate PEM (full chain) |
| `key` | _(none)_ | Path to TLS private-key PEM |

`cert` and `key` must be provided together or both left unset.

### Example `server_config.toml`

```toml
port       = 7777
web_port   = 7778
kill_limit = 20

# map_url = "https://example.com/maps/mymap.tar.zst"

# CA-signed TLS certificate (see §8.2)
# cert = "/etc/letsencrypt/live/play.example.com/fullchain.pem"
# key  = "/etc/letsencrypt/live/play.example.com/privkey.pem"
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

### Locking the server address (server.txt)

`client-web.zip` includes a `server.txt` file. Edit it to contain your server's
address before deploying — the browser client reads it at page load and hides
the address field, exactly like `RSG_SERVER_ADDR` on native builds:

```
# web/server.txt
192.168.1.10:7778
```

Leave the file empty (the default) to show the address field so players can type
their own server address.

### Playing in the browser

1. Extract `client-web.zip`. Edit `server.txt` if desired (see above).
2. Serve the directory with any static web server that supports HTTPS and HTTP/3 (required by WebTransport).
3. Open the URL in Chrome or Edge (Firefox does not yet support WebTransport over self-signed certs).
4. Enter your username and click **CONNECT**.

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
wasm-opt -Oz --strip-debug --vacuum --enable-reference-types \
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

---

## 8. WebTransport Certificate Setup

WebTransport runs over QUIC/HTTP3 and **requires TLS**. The browser refuses to
open a WebTransport session to a server whose certificate it does not trust.
There are two ways to satisfy this requirement:

| | Self-signed (default) | CA-signed (production) |
|---|---|---|
| Setup effort | Zero | Moderate |
| WASM rebuild on restart | **Yes** (every 14 days) | No |
| Works in Chrome / Edge | Yes | Yes |
| Works in Firefox | No¹ | Yes |
| Suitable for | Dev, LAN, short events | Public servers |

¹ Firefox does not currently support WebTransport over self-signed certificates.

---

### 8.1 Self-signed certificate (default)

The server generates a fresh ECDSA P-256 certificate at every startup. The
W3C WebTransport specification caps the validity period at **14 days**, so the
certificate is always short-lived.

**How the browser trusts it**

Instead of a CA chain, the browser validates the certificate by comparing its
SHA-256 fingerprint against a list of trusted digests supplied at connection
time by the client. The fingerprint must therefore be **baked into the WASM
binary at compile time**.

**Workflow**

1. Start the server. It prints the fingerprint:

   ```
   WebTransport listener on [::]:7778 | self-signed cert digest: 4a9f3cABCD...
   → To build the WASM client: RSG_CERT_DIGEST=4a9f3cABCD... cargo build ...
   → Self-signed certificate expires in 14 days; rebuild WASM and restart the server before then.
   ```

2. Compile the WASM client with that digest (see [§7.3](#73-wasm-client)).

3. Every time the server restarts a **new** certificate is generated. You must
   recompile the WASM and redeploy it, then restart the server with the
   new binary at roughly the same time. The 14-day window gives you time to
   plan this; it is not instant.

> **Tip — avoid surprise expiry:** schedule a cron job that rebuilds the WASM
> and restarts the server together before the 14 days are up. A simple
> weekly restart keeps the cert fresh with a comfortable margin.

---

### 8.2 CA-signed certificate (production)

Using a certificate signed by a trusted CA (e.g. Let's Encrypt) removes the
fingerprint constraint entirely. The browser trusts the cert automatically,
no `RSG_CERT_DIGEST` is needed, and the WASM binary never needs to be
recompiled because of a certificate rotation.

**Requirements**

- A domain name pointing to your server (e.g. `play.example.com`).
- A valid TLS certificate for that domain from a recognised CA.
- The certificate PEM file must cover the hostname clients will connect to
  (the Common Name or a Subject Alternative Name entry must match).

**Obtain a certificate with Certbot (Let's Encrypt)**

```bash
# Install certbot
sudo apt-get install certbot       # Debian / Ubuntu
# or: brew install certbot         # macOS

# Issue a certificate (HTTP-01 challenge — requires port 80 to be open briefly)
sudo certbot certonly --standalone -d play.example.com

# Certbot writes the files to:
#   /etc/letsencrypt/live/play.example.com/fullchain.pem   ← certificate chain
#   /etc/letsencrypt/live/play.example.com/privkey.pem     ← private key
```

**Start the server with the certificate**

Add the following to your `server_config.toml`:

```toml
cert = "/etc/letsencrypt/live/play.example.com/fullchain.pem"
key  = "/etc/letsencrypt/live/play.example.com/privkey.pem"
```

Then start the server normally:

```bash
./server
```

The server will log:

```
WebTransport listener on [::]:7778 | using CA-signed certificate
```

**Build the WASM client without a digest**

Leave `RSG_CERT_DIGEST` unset (or set it to an empty string):

```bash
cargo build \
  --target wasm32-unknown-unknown \
  --no-default-features \
  --features web \
  --profile wasm-release \
  --bin client
```

The browser fetches the certificate during the TLS handshake, verifies it
against its built-in CA store, and connects without needing a hardcoded
fingerprint.

**Certificate renewal**

Let's Encrypt certificates are valid for 90 days. Certbot installs a renewal
timer automatically. After renewal, restart the server to pick up the new
files:

```bash
sudo certbot renew
sudo systemctl restart rustshooter-server   # or however you manage the process
```

No WASM rebuild is needed when the cert is renewed.

---

### 8.3 Firewall note for WebTransport

WebTransport uses **QUIC (UDP)** on the WebTransport port (`web_port` in the config,
default 7778). Make sure UDP is open, not just TCP:

```bash
# UFW
sudo ufw allow 7778/udp

# firewalld
sudo firewall-cmd --add-port=7778/udp --permanent && sudo firewall-cmd --reload
```
