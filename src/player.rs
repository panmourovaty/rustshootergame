use bevy::camera::{Camera3dDepthLoadOp, visibility::RenderLayers};
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use avian3d::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::game::{GameState, PlayerProfile};
use crate::input::PlayerAction;
use crate::map::SpawnPoints;
use crate::settings_screen::{GameSettings, rebuild_input_map};
use crate::weapon::Weapon;

pub struct PlayerPlugin;

/// Render layer used exclusively for the local player's weapon model.
/// A dedicated camera renders this layer after clearing depth, so the
/// weapon always draws on top of world geometry without z-fighting.
pub const WEAPON_RENDER_LAYER: usize = 1;

// ─── Components ─────────────────────────────────────────────────────────────

/// Marks the entity driven by the local human player.
#[derive(Component)]
pub struct LocalPlayer;

/// Generic player identification — used for scoring and networking.
#[derive(Component, Clone)]
pub struct Player {
    pub id: u64,
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

/// Marks the camera child entity that renders the player's view.
#[derive(Component)]
pub struct PlayerCamera;

/// Simple FPS controller — replaces bevy_fps_controller for Bevy 0.18 compat.
/// Lives on the logical (physics) player entity.
#[derive(Component)]
pub struct FpsController {
    /// Horizontal look angle (radians). Applied to the body transform.
    pub yaw: f32,
    /// Vertical look angle (radians). Applied to the camera child transform.
    pub pitch: f32,
    /// Ground-movement speed (m/s).
    pub speed: f32,
    /// Ground acceleration (applied per second towards target velocity).
    pub acceleration: f32,
    /// Air-strafing acceleration.
    pub air_acceleration: f32,
    /// Maximum horizontal speed while airborne.
    pub max_air_speed: f32,
    /// Upward impulse applied when jumping.
    pub jump_force: f32,
    /// Mouse sensitivity (radians per pixel).
    pub sensitivity: f32,
    /// Whether keyboard/mouse input is processed (false when cursor is visible).
    pub enable_input: bool,
}

impl Default for FpsController {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            speed: 8.0,
            acceleration: 70.0,
            air_acceleration: 20.0,
            max_air_speed: 6.0,
            jump_force: 7.0,
            sensitivity: 0.002,
            enable_input: false,
        }
    }
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // Spawn after the map and spawn points are ready, once gameplay begins.
        app.add_systems(OnEnter(GameState::Playing), spawn_local_player);
        // Despawn the player entity and release the cursor when returning to lobby.
        app.add_systems(OnEnter(GameState::ConnectScreen), despawn_local_player);
        // Release the cursor and disable input when the game is paused.
        app.add_systems(OnEnter(GameState::Paused), release_cursor_on_pause);
        app.add_systems(
            Update,
            (
                manage_cursor,
                detect_pause_action.after(manage_cursor),
                fps_look.after(detect_pause_action),
                fps_move.after(fps_look),
                handle_respawn,
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}

// ─── Systems ────────────────────────────────────────────────────────────────

fn pick_spawn_point(spawn_points: &SpawnPoints) -> Vec3 {
    let points = &spawn_points.0;
    if points.is_empty() {
        return Vec3::new(0.0, 1.0, 0.0);
    }
    let mut buf = [0u8; 8];
    getrandom::fill(&mut buf).unwrap_or(());
    let idx = u64::from_le_bytes(buf) as usize % points.len();
    points[idx]
}

/// Spawns the local player's physics body and attaches the camera as a child.
/// Also attaches the Leafwing `ActionState` and `InputMap` components so that
/// all gameplay input flows through `PlayerAction`.
pub fn spawn_local_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    spawn_points: Res<SpawnPoints>,
    profile: Res<PlayerProfile>,
    settings: Res<GameSettings>,
) {
    let spawn_pos = pick_spawn_point(&spawn_points);

    // Build InputMap from current GameSettings (supports remapped keybinds).
    let input_map = rebuild_input_map(&settings);

    // Apply mouse sensitivity from settings.
    let mut controller = FpsController::default();
    controller.sensitivity = settings.mouse_sensitivity;

    // Gun meshes — created here, then moved into the camera child closure.
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

    // ── Logical entity: physics body + FPS controller ────────────────────────
    // Collider::capsule(radius, length): radius=0.35 m, cylinder height=1.0 m
    // → total height 1.7 m.  Half-height (for ground-check ray) = 0.85 m.
    // Spawn the core bundle first (Bevy limits spawn tuples to ~15 elements),
    // then insert the Leafwing InputManager components afterwards.
    let player_entity = commands
        .spawn((
            Name::new("LocalPlayerLogical"),
            Transform::from_translation(spawn_pos),
            Visibility::default(),
            RigidBody::Dynamic,
            Collider::capsule(0.35, 1.0),
            LockedAxes::ROTATION_LOCKED,
            LinearVelocity::default(),
            Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
            Restitution::new(0.0).with_combine_rule(CoefficientCombine::Min),
            GravityScale(2.0),
            controller,
            LocalPlayer,
            Player { id: profile.client_id },
            Health::default(),
            Weapon::default(),
        ))
        .id();

    // Insert Leafwing InputManager components in a separate call to stay
    // within Bevy's spawn-tuple limit.
    commands.entity(player_entity).insert((
        ActionState::<PlayerAction>::default(),
        input_map,
    ));

    // Attach camera and weapon children.
    commands
        .entity(player_entity)
        .with_children(|parent| {
            // ── Camera at eye height (0.7 m above capsule centre) ──────────────
            parent
                .spawn((
                    Name::new("LocalPlayerCamera"),
                    Camera3d::default(),
                    Projection::Perspective(PerspectiveProjection {
                        fov: 90.0_f32.to_radians(),
                        ..default()
                    }),
                    Transform::from_xyz(0.0, 0.7, 0.0),
                    PlayerCamera,
                ))
                .with_children(|cam| {
                    // Secondary camera: same position/projection as the main camera
                    // but renders only the weapon layer (layer 1) and clears the
                    // depth buffer first.  This guarantees the weapon always draws
                    // on top of world geometry regardless of how close walls are.
                    cam.spawn((
                        Name::new("WeaponCamera"),
                        Camera3d {
                            depth_load_op: Camera3dDepthLoadOp::Clear(0.0),
                            ..default()
                        },
                        Camera {
                            order: 1,
                            ..default()
                        },
                        Projection::Perspective(PerspectiveProjection {
                            fov: 90.0_f32.to_radians(),
                            ..default()
                        }),
                        RenderLayers::layer(WEAPON_RENDER_LAYER),
                        Transform::default(),
                    ));
                    // Gun body — weapon layer only.
                    cam.spawn((
                        Name::new("GunBody"),
                        Mesh3d(gun_body_mesh),
                        MeshMaterial3d(gun_material),
                        Transform::from_xyz(0.2, -0.15, -0.4),
                        RenderLayers::layer(WEAPON_RENDER_LAYER),
                    ));
                    // Barrel extension — weapon layer only.
                    cam.spawn((
                        Name::new("GunBarrel"),
                        Mesh3d(barrel_mesh),
                        MeshMaterial3d(barrel_material),
                        Transform::from_xyz(0.2, -0.12, -0.63),
                        RenderLayers::layer(WEAPON_RENDER_LAYER),
                    ));
                });
        });
}

/// Left-click locks the cursor and enables FPS input.
///
/// The Escape key is **no longer** handled here — it is bound to the
/// `PlayerAction::Pause` action via Leafwing InputManager and triggers the
/// `detect_pause_action` system which transitions to `GameState::Paused`.
fn manage_cursor(
    mouse_btn: Res<ButtonInput<MouseButton>>,
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
    }
}

/// Transitions to the Paused state when the `Pause` action is pressed during
/// gameplay.  Only fires when input is enabled (cursor is locked) so that the
/// Escape key doesn't accidentally pause when the user hasn't clicked into
/// the game window yet.
fn detect_pause_action(
    action_state: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    controller_query: Query<&FpsController, With<LocalPlayer>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(actions) = action_state.single() else {
        return;
    };
    let Ok(controller) = controller_query.single() else {
        return;
    };

    if controller.enable_input && actions.just_pressed(&PlayerAction::Pause) {
        next_state.set(GameState::Paused);
    }
}

/// Releases the cursor and disables gameplay input when entering the Paused
/// state.  The pause menu (spawned by the settings-screen plugin) needs the
/// cursor visible and free so the user can click Resume / Settings.
fn release_cursor_on_pause(
    mut cursor_query: Query<&mut CursorOptions>,
    mut controller_query: Query<&mut FpsController>,
) {
    for mut cursor in cursor_query.iter_mut() {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
    for mut ctrl in controller_query.iter_mut() {
        ctrl.enable_input = false;
    }
}

/// Reads mouse delta and updates yaw/pitch, then applies them to the body and
/// camera transforms.  Yaw rotates the whole body; pitch only tilts the camera.
fn fps_look(
    mut motion_events: MessageReader<MouseMotion>,
    mut player_query: Query<(&mut FpsController, &mut Transform), With<LocalPlayer>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<LocalPlayer>)>,
) {
    let Ok((mut controller, mut body_tf)) = player_query.single_mut() else {
        return;
    };

    if !controller.enable_input {
        // Drain events so they don't accumulate while paused.
        motion_events.clear();
        return;
    }

    let mut delta = Vec2::ZERO;
    for ev in motion_events.read() {
        delta += ev.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }

    controller.yaw -= delta.x * controller.sensitivity;
    controller.pitch = (controller.pitch - delta.y * controller.sensitivity)
        .clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );

    // Yaw → rotate the physics body around Y.
    body_tf.rotation = Quat::from_rotation_y(controller.yaw);

    // Pitch → tilt the camera child around X.
    if let Ok(mut cam_tf) = camera_query.single_mut() {
        cam_tf.rotation = Quat::from_rotation_x(controller.pitch);
    }
}

/// Reads movement from the Leafwing `ActionState<PlayerAction>` and applies
/// physics forces via avian3d.  Uses `PlayerAction::MoveForward/Back/Left/Right`
/// for horizontal movement and `PlayerAction::Jump` for jumping.
fn fps_move(
    time: Res<Time>,
    mut player_query: Query<
        (
            Entity,
            &FpsController,
            &Transform,
            &mut LinearVelocity,
            &ActionState<PlayerAction>,
        ),
        With<LocalPlayer>,
    >,
    spatial_query: SpatialQuery,
) {
    let Ok((entity, ctrl, body_tf, mut vel, actions)) = player_query.single_mut() else {
        return;
    };
    if !ctrl.enable_input {
        return;
    }

    let dt = time.delta_secs();

    // ── Ground check ─────────────────────────────────────────────────────────
    let ground_filter = SpatialQueryFilter {
        excluded_entities: [entity].into_iter().collect(),
        ..default()
    };
    let is_grounded = spatial_query
        .cast_ray(body_tf.translation, Dir3::NEG_Y, 0.95, true, &ground_filter)
        .is_some();

    // ── Build wish-direction from Leafwing actions (local space) ─────────────
    let mut wish_dir = Vec3::ZERO;
    if actions.pressed(&PlayerAction::MoveForward) {
        wish_dir.z -= 1.0;
    }
    if actions.pressed(&PlayerAction::MoveBack) {
        wish_dir.z += 1.0;
    }
    if actions.pressed(&PlayerAction::MoveLeft) {
        wish_dir.x -= 1.0;
    }
    if actions.pressed(&PlayerAction::MoveRight) {
        wish_dir.x += 1.0;
    }

    if wish_dir.length_squared() > 0.0 {
        wish_dir = (Quat::from_rotation_y(ctrl.yaw) * wish_dir).normalize();
    }

    // ── Apply horizontal acceleration ─────────────────────────────────────────
    let target_speed = if is_grounded { ctrl.speed } else { ctrl.max_air_speed };
    let accel = if is_grounded { ctrl.acceleration } else { ctrl.air_acceleration };

    let current_xz = Vec3::new(vel.x, 0.0, vel.z);
    let target_xz = wish_dir * target_speed;
    let new_xz = current_xz.lerp(target_xz, (accel * dt).min(1.0));

    // ── Jump ──────────────────────────────────────────────────────────────────
    let new_y = if is_grounded && actions.just_pressed(&PlayerAction::Jump) {
        ctrl.jump_force
    } else {
        vel.y
    };

    vel.0 = Vec3::new(new_xz.x, new_y, new_xz.z);
}

/// Teleports the local player back to a random spawn when health reaches zero.
fn handle_respawn(
    mut query: Query<(&mut Health, &mut Transform, &mut LinearVelocity), With<LocalPlayer>>,
    spawn_points: Res<SpawnPoints>,
) {
    for (mut health, mut transform, mut velocity) in query.iter_mut() {
        if health.current <= 0.0 {
            health.current = health.max;
            let respawn = pick_spawn_point(&spawn_points);
            transform.translation = respawn;
            *velocity = LinearVelocity::default();
            info!("Player respawned at {:?}.", respawn);
        }
    }
}

/// Despawns the local player entity (and its camera children) and releases
/// the cursor when returning to the connect screen, so the mouse is usable
/// in the lobby UI.
fn despawn_local_player(
    mut commands: Commands,
    mut cursor_query: Query<&mut CursorOptions>,
    player_query: Query<Entity, With<LocalPlayer>>,
) {
    // Release the cursor first so it's visible and free for UI interaction.
    for mut cursor in cursor_query.iter_mut() {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }

    // Despawn the local player entity (and its camera children).
    for entity in player_query.iter() {
        commands.entity(entity).despawn();
    }
}