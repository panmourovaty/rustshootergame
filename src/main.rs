/// Client binary — shows the connect screen, then runs the full game.
use bevy::prelude::*;
use bevy::window::WindowResolution;

mod connect_screen;
mod game;
mod map;
mod network;
mod player;
mod pvp;
mod ui;
mod weapon;

use connect_screen::ConnectScreenPlugin;
use game::{GamePlugin, GameState};
use map::MapPlugin;
use player::PlayerPlugin;
use pvp::PvpPlugin;
use ui::UiPlugin;
use weapon::WeaponPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RustShooter".to_string(),
                resolution: WindowResolution::new(1280, 720),
                // On WASM, render into the <canvas id="bevy"> element and let it
                // fill the browser viewport.
                #[cfg(target_arch = "wasm32")]
                canvas: Some("#bevy".to_string()),
                #[cfg(target_arch = "wasm32")]
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        // insert_state must come after DefaultPlugins so that the StateTransition
        // schedule (registered by StatesPlugin inside DefaultPlugins) already exists.
        .insert_state(GameState::ConnectScreen)
        .add_plugins(avian3d::PhysicsPlugins::default())
        // GamePlugin calls init_state::<GameState>() which is a no-op here
        // because the state is already inserted above.
        .add_plugins(GamePlugin)
        .add_plugins(ConnectScreenPlugin)
        .add_plugins(MapPlugin)
        .add_plugins(PlayerPlugin)
        .add_plugins(WeaponPlugin)
        .add_plugins(PvpPlugin)
        .add_plugins(UiPlugin)
        .add_plugins(network::client::ClientNetworkPlugin)
        .run();
}
