/// Dedicated-server binary — headless, no rendering, no connect screen.
use bevy::prelude::*;
use clap::Parser;

mod game;
mod map;
mod network;

use game::{GamePlugin, GameState};
use map::MapPlugin;
use network::server::MapUrl;

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
}

fn main() {
    let args = Args::parse();

    App::new()
        .insert_resource(MapUrl(args.map_url))
        .add_plugins(MinimalPlugins)
        // MinimalPlugins omits LogPlugin — add it so info!/warn!/error! work.
        .add_plugins(bevy::log::LogPlugin::default())
        // MinimalPlugins does not include StatesPlugin (only DefaultPlugins does).
        // Add it explicitly so the StateTransition schedule exists before
        // insert_state is called.
        .add_plugins(bevy::state::app::StatesPlugin)
        // Start directly in Loading → Playing; skip ConnectScreen.
        .insert_state(GameState::Loading)
        .add_plugins(avian3d::PhysicsPlugins::default())
        .add_plugins(GamePlugin)
        .add_plugins(MapPlugin)
        .add_plugins(network::server::ServerNetworkPlugin { port: args.port, web_port: args.web_port })
        .add_systems(Startup, print_startup_banner)
        .run();
}
