use crate::game::{GameState, KillEvent, PlayerNames, Scores};
use crate::player::{Health, LocalPlayer};
use crate::weapon::Weapon;
use bevy::prelude::*;

// ─── Game-over timer resource ────────────────────────────────────────────────

/// Counts down after game over; on expiry transitions back to ConnectScreen.
#[derive(Resource)]
struct GameOverTimer(Timer);

pub struct UiPlugin;

// ─── Marker components ───────────────────────────────────────────────────────

#[derive(Component)]
struct HealthText;

#[derive(Component)]
struct AmmoText;

#[derive(Component)]
struct KillFeedRoot;

#[derive(Component)]
struct KillFeedEntry {
    timer: Timer,
}

#[derive(Component)]
struct GameOverScreen;

#[derive(Component)]
struct GameOverCountdownText;

#[derive(Component)]
struct ReloadingText;

/// Marks the root HUD entity so it can be despawned when leaving Playing.
#[derive(Component)]
struct HudRoot;

// ─── Plugin ─────────────────────────────────────────────────────────────────

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        // Spawn HUD only when gameplay starts.
        app.add_systems(OnEnter(GameState::Playing), spawn_hud);
        // Despawn HUD when returning to the connect screen.
        app.add_systems(OnEnter(GameState::ConnectScreen), despawn_hud);
        app.add_systems(
            Update,
            (
                update_health_text,
                update_ammo_text,
                update_reloading_text,
                update_kill_feed,
                append_kill_feed_entries,
            )
                .run_if(in_state(GameState::Playing)),
        );
        app.add_systems(OnEnter(GameState::GameOver), show_game_over_screen);
        app.add_systems(OnExit(GameState::GameOver), hide_game_over_screen);
        app.add_systems(Update, tick_game_over.run_if(in_state(GameState::GameOver)));
    }
}

// ─── HUD layout ─────────────────────────────────────────────────────────────

fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            Name::new("HudRoot"),
            HudRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_crosshair(root);
            spawn_health_display(root);
            spawn_ammo_display(root);
            spawn_kill_feed(root);
            spawn_reloading_text(root);
        });
}

// ── Crosshair ─────────────────────────────────────────────────────────────────

fn spawn_crosshair(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Name::new("CrosshairContainer"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                ..default()
            },
        ))
        .with_children(|c| {
            let white = BackgroundColor(Color::WHITE);

            c.spawn((
                Name::new("CrosshairTop"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-1.0),
                    top: Val::Px(-10.0),
                    width: Val::Px(2.0),
                    height: Val::Px(7.0),
                    ..default()
                },
                white,
            ));
            c.spawn((
                Name::new("CrosshairBottom"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-1.0),
                    top: Val::Px(3.0),
                    width: Val::Px(2.0),
                    height: Val::Px(7.0),
                    ..default()
                },
                white,
            ));
            c.spawn((
                Name::new("CrosshairLeft"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-10.0),
                    top: Val::Px(-1.0),
                    width: Val::Px(7.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                white,
            ));
            c.spawn((
                Name::new("CrosshairRight"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(3.0),
                    top: Val::Px(-1.0),
                    width: Val::Px(7.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                white,
            ));
        });
}

// ── Health display (bottom-left) ──────────────────────────────────────────────

fn spawn_health_display(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Name::new("HealthText"),
        Text::new("HP: 100"),
        TextFont {
            font_size: FontSize::Px(22.0),
            ..default()
        },
        TextColor(Color::srgb(0.2, 1.0, 0.2)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(20.0),
            left: Val::Px(20.0),
            ..default()
        },
        HealthText,
    ));
}

// ── Ammo display (bottom-right) ───────────────────────────────────────────────

fn spawn_ammo_display(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Name::new("AmmoText"),
        Text::new("30 / 30"),
        TextFont {
            font_size: FontSize::Px(22.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.9, 0.2)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(20.0),
            right: Val::Px(20.0),
            ..default()
        },
        AmmoText,
    ));
}

// ── Reloading text (bottom-right, above ammo) ─────────────────────────────────

fn spawn_reloading_text(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Name::new("ReloadingText"),
        Text::new("RELOADING..."),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 0.6, 0.1, 0.0)), // invisible until needed
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(55.0),
            right: Val::Px(20.0),
            ..default()
        },
        ReloadingText,
    ));
}

// ── Kill feed (top-right) ─────────────────────────────────────────────────────

fn spawn_kill_feed(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Name::new("KillFeedRoot"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: Val::Px(4.0),
            ..default()
        },
        KillFeedRoot,
    ));
}

// ─── Update systems ──────────────────────────────────────────────────────────

/// Reads `Health` from the `LocalPlayer` entity (set directly by pvp.rs when
/// the server reports damage, or by weapon.rs for local hit resolution).
fn update_health_text(
    player_query: Query<&Health, With<LocalPlayer>>,
    mut text_query: Query<&mut Text, With<HealthText>>,
) {
    let Ok(health) = player_query.single() else {
        return;
    };
    let Ok(mut text) = text_query.single_mut() else {
        return;
    };
    let hp = health.current.max(0.0) as u32;
    **text = format!("HP: {}", hp);
}

fn update_ammo_text(
    player_query: Query<&Weapon, With<LocalPlayer>>,
    mut text_query: Query<&mut Text, With<AmmoText>>,
) {
    let Ok(weapon) = player_query.single() else {
        return;
    };
    let Ok(mut text) = text_query.single_mut() else {
        return;
    };
    **text = format!("{} / {}", weapon.ammo, weapon.max_ammo);
}

fn update_reloading_text(
    player_query: Query<&Weapon, With<LocalPlayer>>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<ReloadingText>>,
) {
    let Ok(weapon) = player_query.single() else {
        return;
    };
    let Ok((mut text, mut color)) = text_query.single_mut() else {
        return;
    };

    if weapon.is_reloading {
        let pct = 1.0 - (weapon.reload_timer / weapon.reload_duration).clamp(0.0, 1.0);
        **text = format!("RELOADING {:.0}%", pct * 100.0);
        color.0 = Color::srgba(1.0, 0.6, 0.1, 1.0);
    } else {
        color.0 = Color::srgba(1.0, 0.6, 0.1, 0.0);
    }
}

/// Adds a new entry to the kill feed whenever a `KillEvent` fires.
/// Uses `PlayerNames` to display friendly names when available.
fn append_kill_feed_entries(
    mut commands: Commands,
    mut kill_events: MessageReader<KillEvent>,
    feed_query: Query<Entity, With<KillFeedRoot>>,
    scores: Res<Scores>,
    player_names: Res<PlayerNames>,
) {
    let Ok(feed_entity) = feed_query.single() else {
        return;
    };

    for ev in kill_events.read() {
        let killer_name = player_names
            .0
            .get(&ev.killer_id)
            .cloned()
            .unwrap_or_else(|| format!("{}", ev.killer_id & 0xFFFF));
        let victim_name = player_names
            .0
            .get(&ev.victim_id)
            .cloned()
            .unwrap_or_else(|| format!("{}", ev.victim_id & 0xFFFF));

        let killer_kills = scores.get_kills(ev.killer_id);
        let msg = format!(
            "{} eliminated {} [{} kills]",
            killer_name, victim_name, killer_kills
        );

        let entry = commands
            .spawn((
                Name::new("KillFeedEntry"),
                Text::new(msg),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.3, 0.3)),
                Node::default(),
                KillFeedEntry {
                    timer: Timer::from_seconds(5.0, TimerMode::Once),
                },
            ))
            .id();

        commands.entity(feed_entity).add_child(entry);
    }
}

/// Ticks kill-feed entry timers and despawns entries that have expired.
fn update_kill_feed(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut KillFeedEntry)>,
) {
    for (entity, mut entry) in query.iter_mut() {
        entry.timer.tick(time.delta());
        if entry.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn despawn_hud(mut commands: Commands, query: Query<Entity, With<HudRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

// ─── Game-over overlay ───────────────────────────────────────────────────────

const GAME_OVER_DELAY: f32 = 10.0;

fn show_game_over_screen(
    mut commands: Commands,
    scores: Res<Scores>,
    player_names: Res<PlayerNames>,
) {
    let (winner_id, winner_kills) = scores
        .kills
        .iter()
        .max_by_key(|(_, &k)| k)
        .map(|(&id, &k)| (id, k))
        .unwrap_or((0, 0));

    let winner_name = player_names
        .0
        .get(&winner_id)
        .cloned()
        .unwrap_or_else(|| format!("{}", winner_id & 0xFFFF));

    commands.insert_resource(GameOverTimer(Timer::from_seconds(
        GAME_OVER_DELAY,
        TimerMode::Once,
    )));

    commands
        .spawn((
            Name::new("GameOverScreen"),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            GameOverScreen,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("GAME OVER"),
                TextFont {
                    font_size: FontSize::Px(64.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.2, 0.2)),
            ));
            parent.spawn((
                Text::new(format!("{} wins with {} kills!", winner_name, winner_kills)),
                TextFont {
                    font_size: FontSize::Px(32.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                Text::new(format!(
                    "Returning to lobby in {}...",
                    GAME_OVER_DELAY as u32
                )),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
                GameOverCountdownText,
            ));
        });
}

fn tick_game_over(
    time: Res<Time>,
    mut timer: ResMut<GameOverTimer>,
    mut text_query: Query<&mut Text, With<GameOverCountdownText>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    timer.0.tick(time.delta());
    let remaining = (timer.0.remaining_secs().ceil() as u32).max(0);
    for mut text in text_query.iter_mut() {
        **text = format!("Returning to lobby in {}...", remaining);
    }
    if timer.0.just_finished() {
        commands.remove_resource::<GameOverTimer>();
        next_state.set(GameState::ConnectScreen);
    }
}

fn hide_game_over_screen(mut commands: Commands, query: Query<Entity, With<GameOverScreen>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
