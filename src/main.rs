/// Client binary — shows the connect screen, then runs the full game.
use bevy::prelude::*;
use bevy::window::WindowResolution;

#[cfg(not(target_arch = "wasm32"))]
use bevy::render::{
    settings::{RenderCreation, WgpuSettings},
    RenderPlugin,
};

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

const GIT_HASH: &str = env!("GIT_HASH");

// Compile-time label for the graphics API used by this platform build.
#[cfg(target_arch = "wasm32")]
const RENDER_API: &str = "WebGL2";
#[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
const RENDER_API: &str = "Vulkan";
#[cfg(target_os = "windows")]
const RENDER_API: &str = "DirectX 12";
#[cfg(target_os = "macos")]
const RENDER_API: &str = "Metal";

/// Returns the wgpu backend set for this platform.
///
/// Linux:   Vulkan primary, OpenGL ES fallback.
/// Windows: DirectX 12 only (requires Windows 11).
/// macOS:   Metal only.
#[cfg(not(target_arch = "wasm32"))]
fn wgpu_backends() -> wgpu::Backends {
    #[cfg(target_os = "linux")]
    return wgpu::Backends::VULKAN | wgpu::Backends::GL;
    #[cfg(target_os = "windows")]
    return wgpu::Backends::DX12;
    #[cfg(target_os = "macos")]
    return wgpu::Backends::METAL;
}

fn main() {
    let title = format!("RustShooterGame [{}] [{}]", RENDER_API, GIT_HASH);

    // Native: restrict to the platform's preferred graphics API.
    #[cfg(not(target_arch = "wasm32"))]
    let plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                title,
                resolution: WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        })
        .set(RenderPlugin {
            render_creation: RenderCreation::Automatic(WgpuSettings {
                backends: Some(wgpu_backends()),
                ..default()
            }),
            ..default()
        });

    // WASM: render into the <canvas id="bevy"> element; WebGL2 is selected
    // automatically by the bevy/webgl2 feature — no RenderPlugin override needed.
    #[cfg(target_arch = "wasm32")]
    let plugins = DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title,
            canvas: Some("#bevy".to_string()),
            fit_canvas_to_parent: true,
            ..default()
        }),
        ..default()
    });

    App::new()
        .add_plugins(plugins)
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
