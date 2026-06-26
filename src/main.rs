/// Client binary - shows the connect screen, then runs the full game.
use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::window::WindowResolution;

#[cfg(not(target_arch = "wasm32"))]
use bevy::render::{
    settings::{RenderCreation, WgpuSettings},
    RenderPlugin,
};

mod connect_screen;
mod game;
mod map;
mod map_loader;
mod network;
mod player;
mod pvp;
mod ui;
mod weapon;

use connect_screen::ConnectScreenPlugin;
use game::{GamePlugin, GameState};
use map::MapPlugin;
use map_loader::{create_map_asset_source, MapLoaderPlugin};
use player::PlayerPlugin;
use pvp::PvpPlugin;
use ui::UiPlugin;
use weapon::WeaponPlugin;

const GIT_HASH: &str = env!("GIT_HASH");

// Compile-time label for the graphics API used by this platform build.
#[cfg(target_arch = "wasm32")]
const RENDER_API: &str = "WebGPU";
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
fn wgpu_backends() -> bevy::render::settings::Backends {
    #[cfg(target_os = "linux")]
    return bevy::render::settings::Backends::VULKAN | bevy::render::settings::Backends::GL;
    #[cfg(target_os = "windows")]
    return bevy::render::settings::Backends::DX12;
    #[cfg(target_os = "macos")]
    return bevy::render::settings::Backends::METAL;
}

fn main() {
    // ── Map asset source ─────────────────────────────────────────────────────
    // The "map://" source must be registered BEFORE DefaultPlugins (which adds
    // AssetPlugin).  We create a shared Dir here and hand a clone to the
    // MapLoaderPlugin so it can populate files at runtime.
    let (map_source, map_dir) = create_map_asset_source();

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
            render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                backends: Some(wgpu_backends()),
                ..default()
            })),
            ..default()
        });

    // WASM: render into the <canvas id="bevy"> element; WebGPU is selected
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
        .register_asset_source("map", map_source)
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
        .add_plugins(MapLoaderPlugin { dir: map_dir })
        .add_plugins(PlayerPlugin)
        .add_plugins(WeaponPlugin)
        .add_plugins(PvpPlugin)
        .add_plugins(UiPlugin)
        .add_plugins(network::client::ClientNetworkPlugin)
        .run();
}
