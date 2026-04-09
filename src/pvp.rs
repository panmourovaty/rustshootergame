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
                update_remote_player_pos,
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
        // Skip our own join — we are the local player, not a remote one.
        if ev.client_id == profile.client_id {
            continue;
        }
        // Don't double-spawn if we already have this player tracked.
        if remote_players.by_id.contains_key(&ev.client_id) {
            continue;
        }

        let color = Color::srgb(ev.color[0], ev.color[1], ev.color[2]);

        let entity = commands.spawn((
            Name::new(format!("RemotePlayer_{}", ev.client_id)),
            Mesh3d(meshes.add(Cylinder {
                radius: 0.35,
                half_height: 0.85,
            })),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness: 0.8,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.85, 0.0),
            RigidBody::Kinematic,
            Collider::capsule(0.35, 1.0),
            RemotePlayer { client_id: ev.client_id },
        )).id();

        remote_players.by_id.insert(ev.client_id, RemotePlayerData {
            entity,
            username: ev.username.clone(),
        });

        // Register name so the kill feed can display it.
        player_names.0.insert(ev.client_id, ev.username.clone());

        info!(
            "Remote player '{}' (id={}) spawned as {:?}",
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
            info!("Remote player id={} despawned", ev.client_id);
        }
    }
}

fn update_remote_player_pos(
    mut events: MessageReader<RemotePlayerMoved>,
    remote_players: Res<RemotePlayers>,
    mut transform_query: Query<&mut Transform>,
) {
    for ev in events.read() {
        if let Some(data) = remote_players.by_id.get(&ev.client_id) {
            if let Ok(mut tf) = transform_query.get_mut(data.entity) {
                tf.translation = ev.pos;
                tf.rotation = Quat::from_rotation_y(ev.yaw);
            }
        }
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
    mut scores: ResMut<Scores>,
    mut kill_events: MessageWriter<KillEvent>,
) {
    for ev in remote_kill_events.read() {
        scores.add_kill(ev.killer_id);
        scores.add_death(ev.victim_id);

        // Reuse the existing KillEvent so the kill feed picks it up.
        kill_events.write(KillEvent {
            killer_id: ev.killer_id,
            victim_id: ev.victim_id,
        });

        info!(
            "Remote kill confirmed: {} killed {} ({} kills total)",
            ev.killer_id,
            ev.victim_id,
            scores.get_kills(ev.killer_id)
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
