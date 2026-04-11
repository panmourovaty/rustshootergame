use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseButtonInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, MonitorSelection, VideoModeSelection, WindowMode};
use leafwing_input_manager::prelude::*;
use std::collections::HashMap;

use crate::game::GameState;
use crate::input::PlayerAction;
use crate::player::{FpsController, LocalPlayer};

// ─── BindCode ──────────────────────────────────────────────────────────────

/// A single key or mouse-button binding, storable in [`GameSettings::keybinds`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub enum BindCode {
    Key(KeyCode),
    Mouse(MouseButton),
}

impl std::fmt::Display for BindCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindCode::Key(code) => write!(f, "{}", format_keycode(*code)),
            BindCode::Mouse(btn) => write!(f, "{}", format_mouse_button(*btn)),
        }
    }
}

fn format_keycode(code: KeyCode) -> String {
    let s = format!("{code:?}");
    s.strip_prefix("Key").unwrap_or(&s).to_string()
}

fn format_mouse_button(btn: MouseButton) -> &'static str {
    match btn {
        MouseButton::Left => "LMB",
        MouseButton::Right => "RMB",
        MouseButton::Middle => "MMB",
        MouseButton::Back => "Mouse4",
        MouseButton::Forward => "Mouse5",
        _ => "Mouse?",
    }
}

// ─── GameSettings ──────────────────────────────────────────────────────────

/// Persistent game settings shared between the settings screen and gameplay.
#[derive(Resource, Clone, Debug, Reflect)]
pub struct GameSettings {
    pub fullscreen: bool,
    pub antialiasing: bool,
    /// Mouse sensitivity in radians / pixel (mirrored on `FpsController`).
    pub mouse_sensitivity: f32,
    /// One binding per action. Updating a value also updates the
    /// `InputMap<PlayerAction>` on the local-player entity (if it exists).
    pub keybinds: HashMap<PlayerAction, BindCode>,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            fullscreen: false,
            antialiasing: true,
            mouse_sensitivity: 0.002,
            keybinds: default_keybinds(),
        }
    }
}

fn default_keybinds() -> HashMap<PlayerAction, BindCode> {
    use PlayerAction::*;
    [
        (MoveForward, BindCode::Key(KeyCode::KeyW)),
        (MoveBack, BindCode::Key(KeyCode::KeyS)),
        (MoveLeft, BindCode::Key(KeyCode::KeyA)),
        (MoveRight, BindCode::Key(KeyCode::KeyD)),
        (Jump, BindCode::Key(KeyCode::Space)),
        (Shoot, BindCode::Mouse(MouseButton::Left)),
        (Reload, BindCode::Key(KeyCode::KeyR)),
        (Pause, BindCode::Key(KeyCode::Escape)),
        (Scoreboard, BindCode::Key(KeyCode::Tab)),
    ]
    .into_iter()
    .collect()
}

/// Rebuild a complete [`InputMap`] from the current [`GameSettings::keybinds`].
///
/// Used both when spawning the player and when keybinds change at runtime.
pub fn rebuild_input_map(settings: &GameSettings) -> InputMap<PlayerAction> {
    let mut map = InputMap::default();
    for action in ALL_ACTIONS {
        if let Some(bind) = settings.keybinds.get(&action) {
            match bind.clone() {
                BindCode::Key(code) => map.insert(action, code),
                BindCode::Mouse(btn) => map.insert(action, btn),
            };
        }
    }
    map
}

// ─── State management resources ────────────────────────────────────────────

/// Remembers which state to return to when the user clicks Back.
#[derive(Resource)]
pub struct SettingsReturnState(pub GameState);

impl Default for SettingsReturnState {
    fn default() -> Self {
        Self(GameState::Playing)
    }
}

/// While present, the next valid key / mouse press is bound to the stored
/// action.
#[derive(Resource)]
struct ListeningForBind {
    action: PlayerAction,
    /// `false` on the first frame — skips the mouse click that opened the
    /// listener so it isn't accidentally captured as the new binding.
    ready: bool,
}

/// Inserted for one frame when Escape cancels a keybind-listening session.
/// Prevents `handle_settings_escape_key` from also closing the settings
/// screen on the same frame.
#[derive(Resource)]
struct CancelledKeybindListen;

// ─── Constants ──────────────────────────────────────────────────────────────

/// All [`PlayerAction`] variants in a fixed order for the settings UI.
const ALL_ACTIONS: [PlayerAction; 9] = [
    PlayerAction::MoveForward,
    PlayerAction::MoveBack,
    PlayerAction::MoveLeft,
    PlayerAction::MoveRight,
    PlayerAction::Jump,
    PlayerAction::Shoot,
    PlayerAction::Reload,
    PlayerAction::Pause,
    PlayerAction::Scoreboard,
];

/// Returns a friendly label for each [`PlayerAction`] variant.
fn action_label(action: &PlayerAction) -> &'static str {
    match action {
        PlayerAction::MoveForward => "Move Forward",
        PlayerAction::MoveBack => "Move Back",
        PlayerAction::MoveLeft => "Move Left",
        PlayerAction::MoveRight => "Move Right",
        PlayerAction::Jump => "Jump",
        PlayerAction::Shoot => "Shoot",
        PlayerAction::Reload => "Reload",
        PlayerAction::Pause => "Pause",
        PlayerAction::Scoreboard => "Scoreboard",
    }
}

// ─── UI marker components ─────────────────────────────────────────────────

#[derive(Component)]
struct SettingsScreenRoot;
#[derive(Component)]
struct SettingsCamera;

#[derive(Component)]
struct PauseMenuRoot;
#[derive(Component)]
struct PauseMenuCamera;
#[derive(Component)]
struct ResumeButton;
#[derive(Component)]
struct PauseSettingsButton;

#[derive(Component)]
struct FullscreenButton;
#[derive(Component)]
struct AntialiasingButton;
#[derive(Component)]
struct SensitivityValue;
#[derive(Component)]
struct SensitivityDecrease;
#[derive(Component)]
struct SensitivityIncrease;

/// Attached to each keybind row's button. Stores which action this button
/// allows rebinding.
#[derive(Component)]
struct KeybindButton(PlayerAction);

#[derive(Component)]
struct BackButton;

#[derive(Component)]
struct ResetKeybindsButton;

// ─── Plugin ────────────────────────────────────────────────────────────────

pub struct SettingsScreenPlugin;

impl Plugin for SettingsScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameSettings>();
        app.init_resource::<SettingsReturnState>();

        // ── Settings screen ──────────────────────────────────────────────
        app.add_systems(OnEnter(GameState::Settings), spawn_settings_screen);
        app.add_systems(OnExit(GameState::Settings), despawn_settings_screen);
        app.add_systems(
            Update,
            (
                handle_fullscreen_toggle,
                handle_antialiasing_toggle,
                update_toggle_displays,
                handle_sensitivity_decrease,
                handle_sensitivity_increase,
                handle_keybind_click,
                handle_keybind_input,
                update_keybind_displays,
                handle_back_button,
                handle_settings_escape_key,
                handle_reset_keybinds,
            )
                .run_if(in_state(GameState::Settings)),
        );

        // ── Pause menu ───────────────────────────────────────────────────
        app.add_systems(OnEnter(GameState::Paused), spawn_pause_menu);
        app.add_systems(OnExit(GameState::Paused), despawn_pause_menu);
        app.add_systems(
            Update,
            (handle_resume_button, handle_pause_escape_key, handle_pause_settings_button)
                .run_if(in_state(GameState::Paused)),
        );

        // ── Apply settings when entering Playing ─────────────────────────
        app.add_systems(OnEnter(GameState::Playing), apply_settings_to_player);
    }
}

// ─── Settings screen spawn / despawn ────────────────────────────────────────

fn spawn_settings_screen(mut commands: Commands, settings: Res<GameSettings>) {
    commands.spawn((
        Name::new("SettingsCamera"),
        SettingsCamera,
        Camera2d,
    ));

    let on_off = |val: bool| -> &'static str {
        if val { "ON" } else { "OFF" }
    };
    let on_color = |val: bool| -> Color {
        if val {
            Color::srgb(0.1, 0.85, 0.2)
        } else {
            Color::srgb(0.85, 0.1, 0.1)
        }
    };

    commands
        .spawn((
            Name::new("SettingsScreenRoot"),
            SettingsScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("SettingsPanel"),
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    padding: UiRect::all(Val::Px(40.0)),
                    row_gap: Val::Px(12.0),
                    min_width: Val::Px(520.0),
                    max_height: Val::Percent(90.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.02, 0.06, 0.95)),
            ))
            .with_children(|panel| {
                // ── Title ──────────────────────────────────────────────────
                panel.spawn((
                    Text::new("SETTINGS"),
                    TextFont {
                        font_size: 42.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.35, 0.1)),
                    Node {
                        align_self: AlignSelf::Center,
                        margin: UiRect::bottom(Val::Px(16.0)),
                        ..default()
                    },
                ));

                // ── Display section header ────────────────────────────────
                panel.spawn((
                    Text::new("Display"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.7, 0.75)),
                    Node {
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },
                ));

                // ── Fullscreen row ────────────────────────────────────────
                spawn_toggle_row(
                    panel,
                    "Fullscreen",
                    on_off(settings.fullscreen),
                    on_color(settings.fullscreen),
                    FullscreenButton,
                );

                // ── Antialiasing row ──────────────────────────────────────
                spawn_toggle_row(
                    panel,
                    "Antialiasing",
                    on_off(settings.antialiasing),
                    on_color(settings.antialiasing),
                    AntialiasingButton,
                );

                // ── Sensitivity row ────────────────────────────────────────
                panel
                    .spawn((
                        Name::new("SensitivityRow"),
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(12.0),
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new("Mouse Sensitivity"),
                            TextFont {
                                font_size: 18.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            Node {
                                flex_grow: 1.0,
                                ..default()
                            },
                        ));
                        // ◀ decrease
                        row.spawn((
                            Button,
                            SensitivityDecrease,
                            Node {
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.3, 0.3, 0.4)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("<"),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                        // value
                        row.spawn((
                            Text::new(format!("{:.4}", settings.mouse_sensitivity)),
                            TextFont {
                                font_size: 18.0,
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.9, 0.3)),
                            SensitivityValue,
                            Node {
                                min_width: Val::Px(60.0),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                        ));
                        // ▶ increase
                        row.spawn((
                            Button,
                            SensitivityIncrease,
                            Node {
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.3, 0.3, 0.4)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(">"),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    });

                // ── Keybinds section header ────────────────────────────────
                panel.spawn((
                    Text::new("Keybinds"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.7, 0.75)),
                    Node {
                        margin: UiRect::top(Val::Px(16.0)),
                        ..default()
                    },
                ));

                // ── Keybind rows ──────────────────────────────────────────
                for action in ALL_ACTIONS {
                    let label = action_label(&action);
                    let bind_text = settings
                        .keybinds
                        .get(&action)
                        .map(|b: &BindCode| b.to_string())
                        .unwrap_or_else(|| "—".to_string());

                    panel
                        .spawn((
                            Name::new(format!("KeybindRow_{label}")),
                            Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(12.0),
                                ..default()
                            },
                        ))
                        .with_children(|row| {
                            row.spawn((
                                Text::new(label),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                Node {
                                    flex_grow: 1.0,
                                    ..default()
                                },
                            ));
                            row.spawn((
                                Button,
                                KeybindButton(action),
                                Node {
                                    padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    min_width: Val::Px(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.18, 0.18, 0.25)),
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new(bind_text),
                                    TextFont {
                                        font_size: 16.0,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.85, 0.85, 0.85)),
                                ));
                            });
                        });
                }

                // ── Reset keybinds button ──────────────────────────────────
                panel
                    .spawn((
                        Name::new("ResetKeybindsButton"),
                        Button,
                        ResetKeybindsButton,
                        Node {
                            align_self: AlignSelf::Center,
                            padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::top(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.4, 0.25, 0.1)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Reset Keybinds"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.9, 0.8, 0.6)),
                        ));
                    });

                // ── Back button ───────────────────────────────────────────
                panel
                    .spawn((
                        Name::new("BackButton"),
                        Button,
                        BackButton,
                        Node {
                            align_self: AlignSelf::Center,
                            padding: UiRect::axes(Val::Px(40.0), Val::Px(12.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::top(Val::Px(12.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.15, 0.15, 0.22)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("BACK"),
                            TextFont {
                                font_size: 22.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            });
        });
}

/// Helper: spawns a label + ON/OFF toggle button row.
fn spawn_toggle_row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    toggle_text: &str,
    toggle_color: Color,
    marker: impl Component,
) {
    parent
        .spawn((
            Name::new(format!("{label}Row")),
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            row.spawn((
                Button,
                marker,
                Node {
                    padding: UiRect::axes(Val::Px(20.0), Val::Px(6.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    min_width: Val::Px(60.0),
                    ..default()
                },
                BackgroundColor(toggle_color),
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new(toggle_text),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });
}

fn despawn_settings_screen(
    mut commands: Commands,
    root_query: Query<Entity, With<SettingsScreenRoot>>,
    camera_query: Query<Entity, With<SettingsCamera>>,
) {
    for entity in root_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in camera_query.iter() {
        commands.entity(entity).despawn();
    }
}

// ─── Settings toggle systems ────────────────────────────────────────────────

fn handle_fullscreen_toggle(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<FullscreenButton>)>,
    mut settings: ResMut<GameSettings>,
    mut window_query: Query<&mut Window>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            settings.fullscreen = !settings.fullscreen;
            for mut window in window_query.iter_mut() {
                window.mode = if settings.fullscreen {
                    WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
                } else {
                    WindowMode::Windowed
                };
            }
        }
    }
}

fn handle_antialiasing_toggle(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<AntialiasingButton>)>,
    mut settings: ResMut<GameSettings>,
    mut msaa_query: Query<&mut bevy::render::view::Msaa, With<Camera>>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            settings.antialiasing = !settings.antialiasing;
            // Apply the MSAA sample count immediately on all cameras.
            let new_msaa = if settings.antialiasing {
                bevy::render::view::Msaa::Sample4
            } else {
                bevy::render::view::Msaa::Off
            };
            for mut msaa in msaa_query.iter_mut() {
                *msaa = new_msaa;
            }
        }
    }
}

/// Updates the fullscreen and antialiasing toggle button text and background
/// colour to reflect the current `GameSettings` state (e.g. after clicking).
fn update_toggle_displays(
    settings: Res<GameSettings>,
    mut fullscreen_bg: Query<&mut BackgroundColor, With<FullscreenButton>>,
    fullscreen_children: Query<&Children, With<FullscreenButton>>,
    mut aa_bg: Query<&mut BackgroundColor, (With<AntialiasingButton>, Without<FullscreenButton>)>,
    aa_children: Query<&Children, (With<AntialiasingButton>, Without<FullscreenButton>)>,
    mut text_query: Query<&mut Text>,
) {
    if !settings.is_changed() {
        return;
    }

    let on_color = Color::srgb(0.1, 0.85, 0.2);
    let off_color = Color::srgb(0.85, 0.1, 0.1);

    // Fullscreen button
    for mut bg in fullscreen_bg.iter_mut() {
        bg.0 = if settings.fullscreen { on_color } else { off_color };
    }
    if let Ok(children) = fullscreen_children.single() {
        for &child in children {
            if let Ok(mut text) = text_query.get_mut(child) {
                **text = if settings.fullscreen { "ON" } else { "OFF" }.to_string();
            }
        }
    }

    // Antialiasing button
    for mut bg in aa_bg.iter_mut() {
        bg.0 = if settings.antialiasing { on_color } else { off_color };
    }
    if let Ok(children) = aa_children.single() {
        for &child in children {
            if let Ok(mut text) = text_query.get_mut(child) {
                **text = if settings.antialiasing { "ON" } else { "OFF" }.to_string();
            }
        }
    }
}

fn handle_sensitivity_decrease(
    interaction_query: Query<
        &Interaction,
        (Changed<Interaction>, With<SensitivityDecrease>),
    >,
    mut settings: ResMut<GameSettings>,
    mut value_query: Query<&mut Text, With<SensitivityValue>>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            settings.mouse_sensitivity =
                (settings.mouse_sensitivity - 0.0005).max(0.0005);
            if let Ok(mut text) = value_query.single_mut() {
                **text = format!("{:.4}", settings.mouse_sensitivity);
            }
        }
    }
}

fn handle_sensitivity_increase(
    interaction_query: Query<
        &Interaction,
        (Changed<Interaction>, With<SensitivityIncrease>),
    >,
    mut settings: ResMut<GameSettings>,
    mut value_query: Query<&mut Text, With<SensitivityValue>>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            settings.mouse_sensitivity =
                (settings.mouse_sensitivity + 0.0005).min(0.01);
            if let Ok(mut text) = value_query.single_mut() {
                **text = format!("{:.4}", settings.mouse_sensitivity);
            }
        }
    }
}

// ─── Keybind remapping ──────────────────────────────────────────────────────

fn handle_keybind_click(
    interaction_query: Query<
        (&Interaction, &KeybindButton),
        Changed<Interaction>,
    >,
    listening: Option<Res<ListeningForBind>>,
    mut commands: Commands,
) {
    // Ignore clicks while already listening for a bind.
    if listening.is_some() {
        return;
    }
    for (interaction, keybind_btn) in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            commands.insert_resource(ListeningForBind {
                action: keybind_btn.0,
                ready: false,
            });
        }
    }
}

/// Marker to exclude keybind buttons from the click query when we're already
/// listening (the query filters on `Without<ListeningForBindMarker>` which we
/// never actually attach — we instead guard with a `Res<ListeningForBind>`
/// check). This system simply marks `ready = true` on the second frame.
fn handle_keybind_input(
    mut key_events: MessageReader<KeyboardInput>,
    mut mouse_events: MessageReader<MouseButtonInput>,
    mut listening: ResMut<ListeningForBind>,
    mut settings: ResMut<GameSettings>,
    mut input_map_query: Query<&mut InputMap<PlayerAction>, With<LocalPlayer>>,
    keybind_text_query: Query<(&KeybindButton, &Children), With<KeybindButton>>,
    mut text_query: Query<&mut Text>,
    mut commands: Commands,
) {
    // ── Mark ready on the frame after the click ──────────────────────────
    if !listening.ready {
        listening.ready = true;
        // Show "Press a key…" on the button being rebound.
        for (keybind_btn, children) in keybind_text_query.iter() {
            if keybind_btn.0 == listening.action {
                for child in children.iter() {
                    if let Ok(mut text) = text_query.get_mut(child) {
                        **text = "Press a key...".to_string();
                        // We can't easily change TextColor from here without
                        // adding a marker; we just set the text content.
                    }
                }
            }
        }
        // Still drain events so the click that opened the listener isn't
        // captured on the *next* frame.
        for _ in key_events.read() {}
        for _ in mouse_events.read() {}
        return;
    }

    // ── Check keyboard events ────────────────────────────────────────────
    let mut captured = false;

    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        // Ignore Escape — it would conflict with the pause action.
        if ev.logical_key == Key::Escape || ev.key_code == KeyCode::Escape {
            // Cancel the listening state.  Insert a one-frame marker so that
            // `handle_settings_escape_key` doesn't also close the screen.
            commands.remove_resource::<ListeningForBind>();
            commands.insert_resource(CancelledKeybindListen);
            return;
        }
        // Any other key press is captured as the new binding.
        let bind = BindCode::Key(ev.key_code);
        settings.keybinds.insert(listening.action, bind);
        captured = true;
        break;
    }

    // ── Check mouse events ───────────────────────────────────────────────
    if !captured {
        for ev in mouse_events.read() {
            if ev.state != ButtonState::Pressed {
                continue;
            }
            let bind = BindCode::Mouse(ev.button);
            settings.keybinds.insert(listening.action, bind);
            captured = true;
            break;
        }
    }

    if !captured {
        return;
    }

    // ── Update the InputMap on the local-player entity ───────────────────
    if let Ok(mut input_map) = input_map_query.single_mut() {
        *input_map = rebuild_input_map(&settings);
    }

    commands.remove_resource::<ListeningForBind>();
}

fn update_keybind_displays(
    settings: Res<GameSettings>,
    listening: Option<Res<ListeningForBind>>,
    keybind_query: Query<(&KeybindButton, &Children), With<KeybindButton>>,
    mut text_query: Query<&mut Text>,
    mut color_query: Query<&mut TextColor>,
) {
    if !settings.is_changed() && listening.is_none() {
        return;
    }

    for (keybind_btn, children) in keybind_query.iter() {
        let is_listening = listening
            .as_ref()
            .map(|l| l.action == keybind_btn.0 && l.ready)
            .unwrap_or(false);

        for child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                if is_listening {
                    **text = "Press a key...".to_string();
                } else if let Some(bind) = settings.keybinds.get(&keybind_btn.0) {
                    let s: String = bind.to_string();
                    **text = s;
                } else {
                    **text = "—".to_string();
                }
            }
            if is_listening {
                if let Ok(mut color) = color_query.get_mut(child) {
                    color.0 = Color::srgb(1.0, 0.9, 0.2);
                }
            } else {
                if let Ok(mut color) = color_query.get_mut(child) {
                    color.0 = Color::srgb(0.85, 0.85, 0.85);
                }
            }
        }
    }
}

// ─── Back & reset ───────────────────────────────────────────────────────────

fn handle_back_button(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<BackButton>)>,
    return_state: Res<SettingsReturnState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            commands.remove_resource::<ListeningForBind>();
            next_state.set(return_state.0.clone());
        }
    }
}

/// Pressing Escape while in the settings screen goes back (same as Back button).
/// Ignored when listening for a keybind rebind — Escape cancels that instead.
/// Also ignored on the same frame that a keybind listen was just cancelled
/// (tracked by `CancelledKeybindListen`) so we don't close the screen too.
fn handle_settings_escape_key(
    key: Res<ButtonInput<KeyCode>>,
    listening: Option<Res<ListeningForBind>>,
    cancelled: Option<Res<CancelledKeybindListen>>,
    return_state: Res<SettingsReturnState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    // Clean up the one-frame cancellation marker if present.
    if cancelled.is_some() {
        commands.remove_resource::<CancelledKeybindListen>();
    }

    if !key.just_pressed(KeyCode::Escape) {
        return;
    }
    // If we're in the middle of rebinding a key, Escape cancels the listen
    // but does NOT close the settings screen.  The `handle_keybind_input`
    // system handles that case already — just ignore the Escape here.
    if listening.is_some() {
        return;
    }
    // Also bail out if a keybind listen was just cancelled this frame —
    // the user's intent was to cancel the rebind, not close settings.
    if cancelled.is_some() {
        return;
    }
    next_state.set(return_state.0.clone());
}

fn handle_reset_keybinds(
    interaction_query: Query<
        &Interaction,
        (Changed<Interaction>, With<ResetKeybindsButton>),
    >,
    mut settings: ResMut<GameSettings>,
    mut input_map_query: Query<&mut InputMap<PlayerAction>, With<LocalPlayer>>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            settings.keybinds = default_keybinds();
            if let Ok(mut input_map) = input_map_query.single_mut() {
                *input_map = rebuild_input_map(&settings);
            }
        }
    }
}

// ─── Pause menu spawn / despawn ─────────────────────────────────────────────

fn spawn_pause_menu(mut commands: Commands) {
    commands.spawn((
        Name::new("PauseMenuCamera"),
        PauseMenuCamera,
        Camera2d,
    ));

    commands
        .spawn((
            Name::new("PauseMenuRoot"),
            PauseMenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("PausePanel"),
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(48.0)),
                    row_gap: Val::Px(20.0),
                    min_width: Val::Px(320.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.02, 0.06, 0.92)),
            ))
            .with_children(|panel| {
                // ── Title ──────────────────────────────────────────────
                panel.spawn((
                    Text::new("PAUSED"),
                    TextFont {
                        font_size: 48.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        margin: UiRect::bottom(Val::Px(16.0)),
                        ..default()
                    },
                ));

                // ── Resume button ─────────────────────────────────────
                panel
                    .spawn((
                        Name::new("ResumeButton"),
                        Button,
                        ResumeButton,
                        Node {
                            padding: UiRect::axes(Val::Px(48.0), Val::Px(14.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.08, 0.55, 0.12)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("RESUME"),
                            TextFont {
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                // ── Settings button ───────────────────────────────────
                panel
                    .spawn((
                        Name::new("PauseSettingsButton"),
                        Button,
                        PauseSettingsButton,
                        Node {
                            padding: UiRect::axes(Val::Px(48.0), Val::Px(14.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.1, 0.25, 0.6)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("SETTINGS"),
                            TextFont {
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            });
        });
}

fn despawn_pause_menu(
    mut commands: Commands,
    root_query: Query<Entity, With<PauseMenuRoot>>,
    camera_query: Query<Entity, With<PauseMenuCamera>>,
) {
    for entity in root_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in camera_query.iter() {
        commands.entity(entity).despawn();
    }
}

// ─── Pause menu interaction ──────────────────────────────────────────────────

fn handle_resume_button(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<ResumeButton>)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::Playing);
        }
    }
}

/// Pressing Escape while paused resumes the game (same as clicking Resume).
fn handle_pause_escape_key(
    key: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if key.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::Playing);
    }
}

fn handle_pause_settings_button(
    interaction_query: Query<
        &Interaction,
        (Changed<Interaction>, With<PauseSettingsButton>),
    >,
    mut return_state: ResMut<SettingsReturnState>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            // Remember that we came from Paused so Back returns there.
            *return_state = SettingsReturnState(GameState::Paused);
            next_state.set(GameState::Settings);
        }
    }
}

// ─── Apply settings to player ───────────────────────────────────────────────

/// When entering the Playing state, push the current `GameSettings` into the
/// `FpsController` and `InputMap` on the local-player entity and lock the
/// cursor for FPS gameplay.
fn apply_settings_to_player(
    settings: Res<GameSettings>,
    mut player_query: Query<
        (&mut FpsController, &mut InputMap<PlayerAction>),
        With<LocalPlayer>,
    >,
    mut cursor_query: Query<&mut CursorOptions>,
) {
    // Apply mouse sensitivity.
    if let Ok((mut controller, mut input_map)) = player_query.single_mut() {
        controller.sensitivity = settings.mouse_sensitivity;
        controller.enable_input = true;
        *input_map = rebuild_input_map(&settings);
    }

    // Lock cursor.
    for mut cursor in cursor_query.iter_mut() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}