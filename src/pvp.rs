use bevy::prelude::*;
use std::collections::HashMap;
use avian3d::prelude::*;

use crate::game::{GameState, KillEvent, PlayerNames, PlayerProfile, Scores};
use crate::player::Health;

pub struct PvpPlugin;

// ─── Components ─────────────────────────────────────────────────────────────

/// Marks a remotely-controlled player entity spawned by the server.
#[derive(Component, Clone)]
pub struct RemotePlayer {
    pub client_id: u64,
}

/// Interpolation state for a remote player's visual position/rotation.
///
/// Network updates arrive at ~20 Hz (every 50 ms).  Without smoothing the
/// capsule visibly stutters at any framerate above 20 fps.  Each time a new
/// `RemotePlayerMoved` packet arrives we record the current visual position as
/// `from` and the received position as `to`, then advance `elapsed` every
/// frame.  The visual transform is set to `lerp(from, to, t)` where
/// `t = elapsed / INTERP_DURATION`, clamped to 1.0 once we've caught up.
#[derive(Component)]
pub struct RemotePlayerInterp {
    pub from_pos: Vec3,
    pub to_pos: Vec3,
    pub from_rot: Quat,
    pub to_rot: Quat,
    /// Seconds elapsed since the last network update was received.
    pub elapsed: f32,
}

/// How long (in seconds) to spend interpolating between two position snapshots.
/// Two update intervals (2 × 50 ms) gives a small jitter buffer so movement
/// remains smooth even when packets arrive slightly out of time.
const INTERP_DURATION: f32 = 0.1;

// ─── Resources ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct RemotePlayerData {
    pub entity: Entity,
    pub username: String,
}

#[derive(Resource, Default)]
pub struct RemotePlayers {
    pub by_id: HashMap<u64, RemotePlayerData>,
}

// ─── Marker components for UI ────────────────────────────────────────────────

#[derive(Component)]
struct ScoreboardRoot;

#[derive(Component)]
struct ScoreboardText;

// ─── Events ─────────────────────────────────────────────────────────────────

/// Broadcast from server: a new player joined the session.
#[derive(Message, Clone, Debug)]
pub struct RemotePlayerJoined {
    pub client_id: u64,
    pub username: String,
    pub color: [f32; 3],
}

/// Broadcast from server: a player left the session.
#[derive(Message, Clone, Debug)]
pub struct RemotePlayerLeft {
    pub client_id: u64,
}

/// Broadcast from server: a remote player moved.
#[derive(Message, Clone, Debug)]
pub struct RemotePlayerMoved {
    pub client_id: u64,
    pub pos: Vec3,
    pub yaw: f32,
}

/// Server tells us our own HP changed (we took damage).
#[derive(Message, Clone, Debug)]
pub struct LocalPlayerDamaged {
    pub new_hp: f32,
}

/// Server confirmed a kill that happened in the networked session.
#[derive(Message, Clone, Debug)]
pub struct RemoteKillEvent {
    pub killer_id: u64,
    pub victim_id: u64,
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

impl Plugin for PvpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RemotePlayers>();

        app.add_message::<RemotePlayerJoined>();
        app.add_message::<RemotePlayerLeft>();
        app.add_message::<RemotePlayerMoved>();
        app.add_message::<LocalPlayerDamaged>();
        app.add_message::<RemoteKillEvent>();

        app.add_systems(OnEnter(GameState::Playing), (spawn_scoreboard, register_local_player));
        app.add_systems(OnEnter(GameState::ConnectScreen), cleanup_on_disconnect);

        app.add_systems(
            Update,
            (
                spawn_remote_player,
                despawn_remote_player,
                // Receive network snapshots first, then apply smooth interpolation.
                receive_remote_player_pos,
                interpolate_remote_players,
                apply_local_damage,
                handle_remote_kill,
                toggle_scoreboard,
                update_scoreboard,
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}

// ─── Systems ────────────────────────────────────────────────────────────────

fn spawn_remote_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut events: MessageReader<RemotePlayerJoined>,
    mut remote_players: ResMut<RemotePlayers>,
    mut player_names: ResMut<PlayerNames>,
    profile: Res<PlayerProfile>,
) {
    for ev in events.read() {
        warn!(
            "[PVP] RemotePlayerJoined event: id={} username='{}' (local id={})",
            ev.client_id, ev.username, profile.client_id
        );

        // Skip our own join — we are the local player, not a remote one.
        if ev.client_id == profile.client_id {
            warn!("[PVP] Skipping self-join for id={}", ev.client_id);
            continue;
        }
        // Don't double-spawn if we already have this player tracked.
        if remote_players.by_id.contains_key(&ev.client_id) {
            warn!("[PVP] Already tracking id={}, skipping duplicate spawn", ev.client_id);
            continue;
        }

        let color = Color::srgb(ev.color[0], ev.color[1], ev.color[2]);

        // Capsule3d: radius 0.35 m, half_length 0.5 m → total height 1.7 m.
        // Spawn with center at Y = 0.85 so the capsule sits exactly on the floor.
        let spawn_pos = Vec3::new(0.0, 0.85, 0.0);
        let spawn_rot = Quat::IDENTITY;

        // Weapon meshes — same appearance as the local player's gun.
        let gun_body_mesh = meshes.add(Cuboid::new(0.04, 0.08, 0.35));
        let gun_material  = materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.12, 0.12),
            perceptual_roughness: 0.9,
            metallic: 0.6,
            ..default()
        });
        let barrel_mesh     = meshes.add(Cuboid::new(0.02, 0.02, 0.20));
        let barrel_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.08, 0.08),
            metallic: 0.8,
            perceptual_roughness: 0.6,
            ..default()
        });

        // Gun positions are in local space relative to the capsule centre.
        // X=0.45 keeps the mesh clear of the capsule radius (0.35 m).
        // Y=0.40 ≈ shoulder/arm height.  Z=-0.35/-0.58 points the barrel forward.
        let entity = commands.spawn((
            Name::new(format!("RemotePlayer_{}", ev.client_id)),
            Mesh3d(meshes.add(Capsule3d {
                radius: 0.35,
                half_length: 0.5,
            })),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness: 0.6,
                metallic: 0.1,
                ..default()
            })),
            Transform::from_translation(spawn_pos).with_rotation(spawn_rot),
            RigidBody::Kinematic,
            Collider::capsule(0.35, 1.0),
            RemotePlayer { client_id: ev.client_id },
            RemotePlayerInterp {
                from_pos: spawn_pos,
                to_pos: spawn_pos,
                from_rot: spawn_rot,
                to_rot: spawn_rot,
                elapsed: INTERP_DURATION, // already "done" — no movement until first update
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("RemoteGunBody"),
                Mesh3d(gun_body_mesh),
                MeshMaterial3d(gun_material),
                Transform::from_xyz(0.45, 0.40, -0.35),
            ));
            parent.spawn((
                Name::new("RemoteGunBarrel"),
                Mesh3d(barrel_mesh),
                MeshMaterial3d(barrel_material),
                Transform::from_xyz(0.45, 0.43, -0.58),
            ));
        })
        .id();

        remote_players.by_id.insert(ev.client_id, RemotePlayerData {
            entity,
            username: ev.username.clone(),
        });

        // Register name so the kill feed can display it.
        player_names.0.insert(ev.client_id, ev.username.clone());

        warn!(
            "[PVP] Spawned remote player '{}' (id={}) as entity {:?} at (0, 0.85, 0)",
            ev.username, ev.client_id, entity
        );
    }
}

fn despawn_remote_player(
    mut commands: Commands,
    mut events: MessageReader<RemotePlayerLeft>,
    mut remote_players: ResMut<RemotePlayers>,
) {
    for ev in events.read() {
        if let Some(data) = remote_players.by_id.remove(&ev.client_id) {
            commands.entity(data.entity).despawn();
            warn!("[PVP] Remote player id={} despawned", ev.client_id);
        }
    }
}

/// Reads incoming network snapshots and sets the interpolation target.
/// Does NOT write to `Transform` directly — `interpolate_remote_players` does that.
fn receive_remote_player_pos(
    mut events: MessageReader<RemotePlayerMoved>,
    remote_players: Res<RemotePlayers>,
    mut interp_query: Query<(&Transform, &mut RemotePlayerInterp)>,
) {
    for ev in events.read() {
        let Some(data) = remote_players.by_id.get(&ev.client_id) else {
            warn!("[PVP] receive_remote_player_pos: no tracked entity for id={}", ev.client_id);
            continue;
        };
        let Ok((tf, mut interp)) = interp_query.get_mut(data.entity) else {
            warn!("[PVP] receive_remote_player_pos: entity {:?} for id={} missing components", data.entity, ev.client_id);
            continue;
        };
        // Start a new interpolation segment from the current visual position
        // so there is never a visible snap, even if the previous segment was
        // still in progress when this update arrived.
        interp.from_pos = tf.translation;
        interp.from_rot = tf.rotation;
        interp.to_pos   = ev.pos;
        interp.to_rot   = Quat::from_rotation_y(ev.yaw);
        interp.elapsed  = 0.0;
    }
}

/// Advances the interpolation each frame so remote players move smoothly at
/// the full render framerate rather than jumping every network tick.
fn interpolate_remote_players(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut RemotePlayerInterp)>,
) {
    let dt = time.delta_secs();
    for (mut tf, mut interp) in query.iter_mut() {
        interp.elapsed += dt;
        let t = (interp.elapsed / INTERP_DURATION).min(1.0);
        tf.translation = interp.from_pos.lerp(interp.to_pos, t);
        tf.rotation    = interp.from_rot.slerp(interp.to_rot, t);
    }
}

fn apply_local_damage(
    mut events: MessageReader<LocalPlayerDamaged>,
    mut health_query: Query<&mut Health, With<crate::player::LocalPlayer>>,
) {
    for ev in events.read() {
        if let Ok(mut health) = health_query.single_mut() {
            health.current = ev.new_hp;
            info!("Local player HP updated to {:.0}", ev.new_hp);

            // Respawn handled by player.rs handle_respawn when hp <= 0.
        }
    }
}

fn handle_remote_kill(
    mut remote_kill_events: MessageReader<RemoteKillEvent>,
    mut kill_events: MessageWriter<KillEvent>,
) {
    for ev in remote_kill_events.read() {
        // Convert the server-confirmed kill into a local KillEvent.
        // Score updates are handled by `record_kills` — the single source
        // of truth for score mutations — so we only write the event here.
        kill_events.write(KillEvent {
            killer_id: ev.killer_id,
            victim_id: ev.victim_id,
        });

        info!(
            "Remote kill confirmed: {} killed {}",
            ev.killer_id,
            ev.victim_id,
        );
    }
}

/// Called on OnEnter(Playing): record the local player's own name so the
/// scoreboard can display it (the server doesn't send us our own PlayerJoinMsg
/// in a way that would reach spawn_remote_player after the self-filter).
fn register_local_player(
    profile: Res<PlayerProfile>,
    mut player_names: ResMut<PlayerNames>,
) {
    player_names.0.insert(profile.client_id, profile.username.clone());
}

// ─── Scoreboard ──────────────────────────────────────────────────────────────

fn spawn_scoreboard(mut commands: Commands) {
    commands
        .spawn((
            Name::new("ScoreboardRoot"),
            ScoreboardRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(25.0),
                top: Val::Percent(15.0),
                width: Val::Percent(50.0),
                padding: UiRect::all(Val::Px(16.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // Title row
            root.spawn((
                Name::new("ScoreboardTitle"),
                Text::new("SCOREBOARD"),
                TextFont {
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            // Header
            root.spawn((
                Name::new("ScoreboardHeader"),
                Text::new("Name                    K    D"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.5)),
            ));
            // Content (rebuilt every frame the board is visible)
            root.spawn((
                Name::new("ScoreboardText"),
                Text::new(""),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
                ScoreboardText,
            ));
        });
}

fn toggle_scoreboard(
    key: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Visibility, With<ScoreboardRoot>>,
) {
    if key.just_pressed(KeyCode::Tab) {
        for mut vis in query.iter_mut() {
            *vis = match *vis {
                Visibility::Hidden => Visibility::Visible,
                _ => Visibility::Hidden,
            };
        }
    }
}

fn update_scoreboard(
    board_query: Query<&Visibility, With<ScoreboardRoot>>,
    mut text_query: Query<&mut Text, With<ScoreboardText>>,
    scores: Res<Scores>,
    player_names: Res<PlayerNames>,
    remote_players: Res<RemotePlayers>,
) {
    // Only rebuild when visible.
    let is_visible = board_query
        .iter()
        .any(|v| !matches!(v, Visibility::Hidden));
    if !is_visible {
        return;
    }

    let Ok(mut text) = text_query.single_mut() else {
        return;
    };

    // Collect all known player IDs from scores and remote players.
    let mut all_ids: Vec<u64> = scores.kills.keys().copied()
        .chain(scores.deaths.keys().copied())
        .chain(remote_players.by_id.keys().copied())
        .collect();
    all_ids.sort_unstable();
    all_ids.dedup();

    // Sort by kills descending.
    all_ids.sort_by(|a, b| {
        scores.get_kills(*b).cmp(&scores.get_kills(*a))
    });

    let mut lines = String::new();
    for id in all_ids {
        let name = player_names.0
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("Player {}", id & 0xFFFF));
        let kills = scores.get_kills(id);
        let deaths = scores.get_deaths(id);
        lines.push_str(&format!("{:<24} {:<5}{}\n", name, kills, deaths));
    }

    **text = lines;
}

/// Called on OnEnter(ConnectScreen): clears all remote player state and
/// despawns their entities so the next session starts clean.
fn cleanup_on_disconnect(
    mut commands: Commands,
    mut remote_players: ResMut<RemotePlayers>,
    scoreboard_query: Query<Entity, With<ScoreboardRoot>>,
) {
    for (_, data) in remote_players.by_id.drain() {
        commands.entity(data.entity).despawn();
    }
    for entity in scoreboard_query.iter() {
        commands.entity(entity).despawn();
    }
    info!("Remote players cleared on disconnect.");
}
