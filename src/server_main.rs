/// Dedicated-server binary — headless, no rendering, no connect screen.
use bevy::prelude::*;
use clap::Parser;
use std::path::PathBuf;

mod game;
mod map;
mod network;

use game::{GamePlugin, GameState};
use map::MapPlugin;
use network::server::{MapUrl, ServerIdentity};

fn print_startup_banner(ports: Res<network::server::ServerPorts>) {
    info!("===========================================");
    info!(" RustShooter dedicated server is running  ");
    info!("===========================================");
    info!(" UDP  (native clients)  : 0.0.0.0:{}", ports.udp);
    info!(" WebTransport (browser) : 0.0.0.0:{}", ports.web);
    info!("===========================================");
}

#[derive(Parser, Debug)]
#[command(name = "server", about = "RustShooter dedicated server")]
struct Args {
    /// UDP port for native clients.
    #[arg(long, default_value_t = 7777)]
    port: u16,
    /// WebTransport port for browser (WASM) clients.
    #[arg(long, default_value_t = 7778)]
    web_port: u16,
    /// HTTPS URL of the map archive (.tar.zst) to send to connecting clients.
    /// If omitted, clients use the built-in placeholder map.
    ///
    /// Example: --map-url https://example.com/maps/mymap.tar.zst
    #[arg(long)]
    map_url: Option<String>,
    /// Path to the TLS certificate PEM file (full chain) for WebTransport.
    /// Must be provided together with --key.
    /// When omitted the server generates an ephemeral self-signed certificate.
    #[arg(long, requires = "key")]
    cert: Option<PathBuf>,
    /// Path to the TLS private-key PEM file for WebTransport.
    /// Must be provided together with --cert.
    #[arg(long, requires = "cert")]
    key: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    // Build the TLS identity before starting Bevy.
    // Identity::load_pemfiles is async; use a minimal single-threaded tokio
    // runtime just to drive that one future.
    let identity = if let (Some(cert), Some(key)) = (args.cert, args.key) {
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
        .insert_resource(MapUrl(args.map_url))
        .insert_resource(identity)
        .add_plugins(MinimalPlugins)
        // MinimalPlugins omits LogPlugin — add it so info!/warn!/error! work.
        .add_plugins(bevy::log::LogPlugin::default())
        // MinimalPlugins does not include StatesPlugin (only DefaultPlugins does).
        // Add it explicitly so the StateTransition schedule exists before
        // insert_state is called.
        .add_plugins(bevy::state::app::StatesPlugin)
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
        .add_plugins(network::server::ServerNetworkPlugin { port: args.port, web_port: args.web_port })
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
