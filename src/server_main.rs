/// Dedicated-server binary — headless, no rendering, no connect screen.
use bevy::prelude::*;
use clap::Parser;

mod game;
mod map;
mod network;

use game::{GamePlugin, GameState};
use map::MapPlugin;

#[derive(Parser, Debug)]
#[command(name = "server", about = "RustShooter dedicated server")]
struct Args {
    /// UDP port to listen on.
    #[arg(long, default_value_t = 7777)]
    port: u16,
}

fn main() {
    let args = Args::parse();

    App::new()
        // Start directly in Loading → Playing; skip ConnectScreen.
        .insert_state(GameState::Loading)
        .add_plugins(MinimalPlugins)
        .add_plugins(avian3d::PhysicsPlugins::default())
        .add_plugins(GamePlugin)
        .add_plugins(MapPlugin)
        .add_plugins(network::server::ServerNetworkPlugin { port: args.port })
        .run();
}
