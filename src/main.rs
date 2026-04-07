use bevy::prelude::*;
use bevy::window::WindowResolution;
use clap::Parser;

mod game;
mod map;
mod network;
mod player;
mod ui;
mod weapon;

use game::GamePlugin;
use map::MapPlugin;
use player::PlayerPlugin;
use ui::UiPlugin;
use weapon::WeaponPlugin;

#[derive(Parser, Debug)]
#[command(name = "rustshootergame", about = "A Counter Strike-like FPS game")]
struct Args {
    /// Run as a dedicated server (no window, no rendering)
    #[arg(long)]
    server: bool,
    /// Connect to a remote server as a network client
    #[arg(long)]
    client: bool,
    /// Server hostname / IP to connect to (client mode only)
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    /// UDP port used by server and client
    #[arg(long, default_value_t = 7777)]
    port: u16,
}

fn main() {
    let args = Args::parse();

    let mut app = App::new();

    if args.server {
        // ── Dedicated server ─────────────────────────────────────────────────
        app.add_plugins(MinimalPlugins);
        app.add_plugins(avian3d::PhysicsPlugins::default());
        app.add_plugins(GamePlugin);
        app.add_plugins(MapPlugin);

        #[cfg(feature = "networking")]
        {
            let port = args.port;
            app.add_plugins(network::server::ServerNetworkPlugin { port });
        }
    } else {
        // ── Demo / client mode — renders the game ────────────────────────────
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RustShooter".to_string(),
                resolution: WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }));
        app.add_plugins(avian3d::PhysicsPlugins::default());
        app.add_plugins(GamePlugin);
        app.add_plugins(MapPlugin);
        app.add_plugins(PlayerPlugin);
        app.add_plugins(WeaponPlugin);
        app.add_plugins(UiPlugin);

        if args.client {
            #[cfg(feature = "networking")]
            {
                let host = args.host.clone();
                let port = args.port;
                app.add_plugins(network::client::ClientNetworkPlugin {
                    server_addr: format!("{}:{}", host, port)
                        .parse()
                        .expect("Invalid server address"),
                });
            }
        }
    }

    app.run();
}
