use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use avian3d::prelude::*;
use bevy_fps_controller::controller::*;
use crate::map::SpawnPoints;
use crate::weapon::Weapon;

pub struct PlayerPlugin;

// ─── Components ─────────────────────────────────────────────────────────────

/// Marks the entity driven by the local human player.
#[derive(Component)]
pub struct LocalPlayer;

/// Generic player identification — used for scoring and networking.
#[derive(Component, Clone)]
pub struct Player {
    pub id: u32,
}

/// Hit-point component shared by all damage-able entities.
#[derive(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            current: 100.0,
            max: 100.0,
        }
    }
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FpsControllerPlugin);
        app.add_systems(Startup, spawn_local_player);
        app.add_systems(Update, (manage_cursor, handle_respawn));
    }
}

// ─── Systems ────────────────────────────────────────────────────────────────

/// Spawns the local player's logical physics entity and the camera render entity.
pub fn spawn_local_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    spawn_points: Res<SpawnPoints>,
) {
    let spawn_pos = spawn_points.0[0];

    // ── Logical entity: physics + FPS controller ─────────────────────────────
    // avian3d 0.4: Collider::capsule(radius, length) where length is the
    // distance between the two hemisphere centres (the cylindrical section height).
    // Total capsule height = length + 2 * radius.
    // radius = 0.35 m, length = 1.0 m → total height ≈ 1.7 m.
    let logical_entity = commands
        .spawn((
            Name::new("LocalPlayerLogical"),
            Transform::from_translation(spawn_pos),
            // Avian physics
            RigidBody::Dynamic,
            Collider::capsule(0.35, 1.0),
            LockedAxes::ROTATION_LOCKED,
            LinearVelocity::default(),
            Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
            Restitution::new(0.0).with_combine_rule(CoefficientCombine::Min),
            GravityScale(2.0),
            // FPS controller — input starts disabled until cursor is locked
            FpsController {
                air_acceleration: 80.0,
                acceleration: 70.0,
                max_air_speed: 20.0,
                enable_input: false,
                ..default()
            },
            CameraConfig {
                height_offset: 0.0,
            },
            // Game logic
            LocalPlayer,
            Player { id: 0 },
            Health::default(),
            Weapon::default(),
        ))
        .id();

    // ── Gun meshes ────────────────────────────────────────────────────────────
    let gun_body_mesh = meshes.add(Cuboid::new(0.04, 0.08, 0.35));
    let gun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.12, 0.12),
        perceptual_roughness: 0.9,
        metallic: 0.6,
        ..default()
    });
    let barrel_mesh = meshes.add(Cuboid::new(0.02, 0.02, 0.20));
    let barrel_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.08),
        metallic: 0.8,
        perceptual_roughness: 0.6,
        ..default()
    });

    // ── Camera / render entity ────────────────────────────────────────────────
    commands
        .spawn((
            Name::new("LocalPlayerCamera"),
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                fov: 90.0_f32.to_radians(),
                ..default()
            }),
            RenderPlayer { logical_entity },
        ))
        .with_children(|parent| {
            // Gun body (bottom-right of screen)
            parent.spawn((
                Name::new("GunBody"),
                Mesh3d(gun_body_mesh),
                MeshMaterial3d(gun_material),
                Transform::from_xyz(0.2, -0.15, -0.4),
            ));
            // Barrel extension
            parent.spawn((
                Name::new("GunBarrel"),
                Mesh3d(barrel_mesh),
                MeshMaterial3d(barrel_material),
                Transform::from_xyz(0.2, -0.12, -0.63),
            ));
        });
}

/// Left-click to lock the cursor and enable FPS movement; Escape to release.
fn manage_cursor(
    mouse_btn: Res<ButtonInput<MouseButton>>,
    key: Res<ButtonInput<KeyCode>>,
    mut cursor_query: Query<&mut CursorOptions>,
    mut controller_query: Query<&mut FpsController>,
) {
    let Ok(mut cursor) = cursor_query.single_mut() else {
        return;
    };

    if mouse_btn.just_pressed(MouseButton::Left) && cursor.visible {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
        for mut ctrl in controller_query.iter_mut() {
            ctrl.enable_input = true;
        }
        return;
    }

    if key.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        for mut ctrl in controller_query.iter_mut() {
            ctrl.enable_input = false;
        }
    }
}

/// When the local player's health drops to zero, teleport them back to spawn
/// and reset their health.
fn handle_respawn(
    mut query: Query<(&mut Health, &mut Transform, &mut LinearVelocity), With<LocalPlayer>>,
    spawn_points: Res<SpawnPoints>,
) {
    for (mut health, mut transform, mut velocity) in query.iter_mut() {
        if health.current <= 0.0 {
            health.current = health.max;
            transform.translation = spawn_points.0[0];
            *velocity = LinearVelocity::default();
            info!("Player respawned at {:?}.", spawn_points.0[0]);
        }
    }
}
