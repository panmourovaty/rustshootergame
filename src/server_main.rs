/// Dedicated-server binary — headless, no rendering, no connect screen.
use bevy::prelude::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};

mod game;
mod input;
mod map;
mod network;

use game::{GameConfig, GamePlugin, GameState};
use map::MapPlugin;
use network::server::{MapUrl, ServerIdentity};

// ─── Configuration ───────────────────────────────────────────────────────────

/// All server settings read from `server_config.toml` (or the path given as
/// the first command-line argument).
///
/// Every field is optional in the file; unset fields fall back to the defaults
/// shown in `example_server_config.toml`.
#[derive(Deserialize)]
#[serde(default)]
struct ServerConfig {
    /// UDP port for native (desktop) clients.
    port: u16,
    /// WebTransport (HTTP/3) port for browser (WASM) clients.
    web_port: u16,
    /// HTTPS URL of the map archive (`.tar.zst`) to serve to connecting clients.
    /// When absent, clients use the built-in placeholder map.
    map_url: Option<String>,
    /// Number of kills required for the game to end.
    kill_limit: u32,
    /// Path to the TLS certificate PEM file (full chain) for WebTransport.
    /// Must be supplied together with `key`; when both are absent an ephemeral
    /// self-signed certificate is generated instead.
    cert: Option<PathBuf>,
    /// Path to the TLS private-key PEM file for WebTransport.
    /// Must be supplied together with `cert`.
    key: Option<PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 7777,
            web_port: 7778,
            map_url: None,
            kill_limit: 20,
            cert: None,
            key: None,
        }
    }
}

/// Loads `ServerConfig` from:
/// 1. The path given as the first CLI argument, if any.
/// 2. `server_config.toml` in the current directory, if it exists.
/// 3. Hard-coded defaults if neither of the above is found.
fn load_config() -> ServerConfig {
    let explicit_path = std::env::args().nth(1);

    let path_str = match explicit_path {
        Some(ref p) => p.as_str(),
        None => "server_config.toml",
    };
    let path = Path::new(path_str);

    if path.exists() {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("ERROR: could not read config file '{}': {}", path_str, e);
                std::process::exit(1);
            }
        };
        match toml::from_str::<ServerConfig>(&text) {
            Ok(cfg) => {
                info!("Loaded server config from '{}'.", path_str);
                cfg
            }
            Err(e) => {
                eprintln!("ERROR: failed to parse config file '{}': {}", path_str, e);
                std::process::exit(1);
            }
        }
    } else {
        if explicit_path.is_some() {
            // User gave a path that doesn't exist — that's clearly a mistake.
            eprintln!("ERROR: config file '{}' not found.", path_str);
            std::process::exit(1);
        }
        // No explicit path, no default file — silently use built-in defaults.
        info!("No server_config.toml found; using built-in defaults.");
        ServerConfig::default()
    }
}

// ─── Startup banner ──────────────────────────────────────────────────────────

fn print_startup_banner(ports: Res<network::server::ServerPorts>) {
    info!("===========================================");
    info!(" RustShooter dedicated server is running  ");
    info!("===========================================");
    info!(" UDP  (native clients)  : 0.0.0.0:{}", ports.udp);
    info!(" WebTransport (browser) : 0.0.0.0:{}", ports.web);
    info!("===========================================");
}

// ─── Entry point ─────────────────────────────────────────────────────────────

fn main() {
    let config = load_config();

    // Validate cert + key: both must be present or both absent.
    let has_cert = config.cert.is_some();
    let has_key = config.key.is_some();
    if has_cert != has_key {
        eprintln!("ERROR: 'cert' and 'key' must both be set or both be absent in the config.");
        std::process::exit(1);
    }

    // Build the TLS identity before starting Bevy.
    // Identity::load_pemfiles is async; use a minimal single-threaded tokio
    // runtime just to drive that one future.
    let identity = if let (Some(cert), Some(key)) = (config.cert, config.key) {
        use aeronet_webtransport::wtransport::Identity;
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("tokio runtime");
        let id = rt
            .block_on(Identity::load_pemfiles(cert, key))
            .expect("failed to load TLS certificate from PEM files");
        ServerIdentity { identity: id, self_signed_digest: None }
    } else {
        use aeronet_webtransport::wtransport::Identity;
        let id = Identity::self_signed(["localhost", "127.0.0.1", "::1"])
            .expect("failed to generate self-signed TLS certificate");
        let dotted = format!("{}", id.certificate_chain().as_slice()[0].hash());
        let digest = dotted.replace(':', "");
        ServerIdentity { identity: id, self_signed_digest: Some(digest) }
    };

    App::new()
        // Insert GameConfig before GamePlugin so init_resource keeps our value.
        .insert_resource(GameConfig { kill_limit: config.kill_limit })
        .insert_resource(MapUrl(config.map_url))
        .insert_resource(identity)
        .add_plugins(MinimalPlugins)
        // MinimalPlugins omits LogPlugin — add it so info!/warn!/error! work.
        .add_plugins(bevy::log::LogPlugin::default())
        // MinimalPlugins does not include StatesPlugin (only DefaultPlugins does).
        // Add it explicitly so the StateTransition schedule exists before
        // insert_state is called.
        .add_plugins(bevy::state::app::StatesPlugin)
        // InputPlugin is needed by lightyear_inputs_leafwing so that
        // InputManagerPlugin::server() can register keyboard/mouse input
        // types for reflection and deserialization.
        .add_plugins(bevy::input::InputPlugin)
        // MinimalPlugins omits AssetPlugin, but avian3d's collider-from-mesh
        // feature registers a `clear_unused_colliders` system that reads
        // MessageReader<AssetEvent<Mesh>>.  Without AssetPlugin the message
        // channel is never initialised and Bevy panics at startup.  Adding
        // AssetPlugin (headless, no renderer required) and registering Mesh as
        // an asset type satisfies avian3d even though the server never loads
        // mesh assets — it uses only explicit Collider primitives.
        .add_plugins(bevy::asset::AssetPlugin::default())
        .init_asset::<Mesh>()
        .add_plugins(avian3d::PhysicsPlugins::default())
        .add_plugins(GamePlugin)
        .add_plugins(MapPlugin)
        .add_plugins(network::server::ServerNetworkPlugin {
            port: config.port,
            web_port: config.web_port,
        })
        // Skip ConnectScreen: immediately advance to Loading (which GamePlugin's
        // OnEnter(Loading) system then advances to Playing).  Done via a startup
        // system so GamePlugin's init_state::<GameState>() runs first and we
        // avoid the "already initialized" warning from double-inserting state.
        .add_systems(Startup, |mut next: ResMut<NextState<GameState>>| {
            next.set(GameState::Loading);
        })
        .add_systems(Startup, print_startup_banner)
        .run();
}
