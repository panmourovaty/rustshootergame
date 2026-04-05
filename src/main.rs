mod app;
mod components;
mod fps_controller;
mod networking;
mod physics;
mod shooting;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Rust Shooter Game".into(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(networking::NetworkingPlugin)
        .add_plugins(app::setup())
        .add_plugins(app::setup_game())
        .run();
}
