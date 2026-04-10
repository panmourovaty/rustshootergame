use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use crate::game::{
    ConnectionError, GameSettings, GameState, MsaaSetting,
    PlayerProfile, SENSITIVITY_LABELS, SENSITIVITY_STEPS,
};

pub struct ConnectScreenPlugin;

// ─── Components ─────────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum InputField {
    Username,
    ServerIp,
}

/// Marks the text node that mirrors the live value of an input field.
#[derive(Component)]
struct FieldDisplay(InputField);

#[derive(Component)]
struct ConnectScreenRoot;

#[derive(Component)]
struct ConnectScreenCamera;

#[derive(Component)]
struct ConnectButton;

#[derive(Component)]
struct ErrorText;

/// Main lobby panel (username / server / connect).
#[derive(Component)]
struct ConnectMainPanel;

/// Settings overlay panel — hidden by default.
#[derive(Component)]
struct SettingsPanel;

#[derive(Component)]
struct SettingsButton;

#[derive(Component)]
struct BackFromSettingsButton;

#[derive(Component)]
struct SensDecBtn;

#[derive(Component)]
struct SensIncBtn;

/// Plain text showing the current sensitivity label between ◄ ►.
#[derive(Component)]
struct SensDisplay;

/// Button that cycles through AA modes; its child text is AaDisplay.
#[derive(Component)]
struct AaCycleBtn;

#[derive(Component)]
struct AaDisplay;

/// Button that toggles fullscreen; its child text is FullscreenDisplay.
#[derive(Component)]
struct FullscreenToggleBtn;

#[derive(Component)]
struct FullscreenDisplay;

// ─── Connecting overlay components ──────────────────────────────────────────

#[derive(Component)]
struct ConnectingScreenRoot;

#[derive(Component)]
struct ConnectingScreenCamera;

#[derive(Component)]
struct CancelButton;

// ─── Resources ──────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
struct FocusedField(Option<InputField>);

// ─── Plugin ─────────────────────────────────────────────────────────────────

impl Plugin for ConnectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FocusedField>();

        // Connect screen
        app.add_systems(OnEnter(GameState::ConnectScreen), spawn_connect_screen);
        app.add_systems(OnExit(GameState::ConnectScreen), despawn_connect_screen);
        app.add_systems(
            Update,
            (
                handle_field_click,
                handle_text_input,
                update_field_display,
                update_field_highlight,
                handle_connect_button,
                handle_settings_button,
                handle_back_button,
                handle_sens_dec,
                handle_sens_inc,
                handle_aa_cycle,
                handle_fullscreen_toggle,
                update_settings_displays,
            )
                .run_if(in_state(GameState::ConnectScreen)),
        );

        // Connecting overlay
        app.add_systems(OnEnter(GameState::Connecting), spawn_connecting_screen);
        app.add_systems(OnExit(GameState::Connecting), despawn_connecting_screen);
        app.add_systems(
            Update,
            handle_cancel_button.run_if(in_state(GameState::Connecting)),
        );
    }
}

// ─── Connect screen spawn/despawn ────────────────────────────────────────────

fn spawn_connect_screen(
    mut commands: Commands,
    profile: Res<PlayerProfile>,
    conn_error: Res<ConnectionError>,
    settings: Res<GameSettings>,
) {
    commands.spawn((
        Name::new("ConnectScreenCamera"),
        ConnectScreenCamera,
        Camera2d,
    ));

    commands
        .spawn((
            Name::new("ConnectScreenRoot"),
            ConnectScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.04, 0.04, 0.10)),
        ))
        .with_children(|root| {
            // ── Main connect panel ──────────────────────────────────────────
            root.spawn((
                Name::new("MainPanel"),
                ConnectMainPanel,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    padding: UiRect::all(Val::Px(48.0)),
                    row_gap: Val::Px(12.0),
                    min_width: Val::Px(420.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.88)),
            ))
            .with_children(|panel| {
                // ── Title ──────────────────────────────────────────────────
                panel.spawn((
                    Text::new("RUST SHOOTER"),
                    TextFont { font_size: 46.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.30, 0.10)),
                    Node {
                        align_self: AlignSelf::Center,
                        margin: UiRect::bottom(Val::Px(24.0)),
                        ..default()
                    },
                ));

                // ── Username ────────────────────────────────────────────────
                panel.spawn((
                    Text::new("Username"),
                    TextFont { font_size: 15.0, ..default() },
                    TextColor(Color::srgb(0.72, 0.72, 0.72)),
                ));
                panel
                    .spawn((
                        Name::new("UsernameInput"),
                        Button,
                        InputField::Username,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(42.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.14, 0.14, 0.20)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new(profile.username.clone() + "|"),
                            TextFont { font_size: 20.0, ..default() },
                            TextColor(Color::WHITE),
                            FieldDisplay(InputField::Username),
                        ));
                    });

                // ── Server address (hidden when RSG_SERVER_ADDR env var is set) ─
                if !profile.server_addr_locked {
                    panel.spawn((
                        Text::new("Server Address"),
                        TextFont { font_size: 15.0, ..default() },
                        TextColor(Color::srgb(0.72, 0.72, 0.72)),
                        Node { margin: UiRect::top(Val::Px(8.0)), ..default() },
                    ));
                    panel
                        .spawn((
                            Name::new("ServerInput"),
                            Button,
                            InputField::ServerIp,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(42.0),
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.14, 0.14, 0.20)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(profile.server_addr.clone() + " "),
                                TextFont { font_size: 20.0, ..default() },
                                TextColor(Color::WHITE),
                                FieldDisplay(InputField::ServerIp),
                            ));
                        });
                }

                // ── Error line ─────────────────────────────────────────────
                let error_msg = conn_error.0.clone().unwrap_or_default();
                panel.spawn((
                    Name::new("ErrorText"),
                    Text::new(error_msg),
                    TextFont { font_size: 14.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.30, 0.30)),
                    Node { min_height: Val::Px(18.0), ..default() },
                    ErrorText,
                ));

                // ── Button row: Connect  ⚙ Settings ────────────────────────
                panel
                    .spawn((
                        Name::new("ButtonRow"),
                        Node {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(12.0),
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Name::new("ConnectButton"),
                            Button,
                            ConnectButton,
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
                                Text::new("CONNECT"),
                                TextFont { font_size: 24.0, ..default() },
                                TextColor(Color::WHITE),
                            ));
                        });

                        row.spawn((
                            Name::new("SettingsButton"),
                            Button,
                            SettingsButton,
                            Node {
                                padding: UiRect::axes(Val::Px(20.0), Val::Px(14.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.20, 0.20, 0.30)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("Settings"),
                                TextFont { font_size: 20.0, ..default() },
                                TextColor(Color::srgb(0.85, 0.85, 0.85)),
                            ));
                        });
                    });

                // ── Hint ───────────────────────────────────────────────────
                panel.spawn((
                    Text::new("Click a field to type - Enter or click CONNECT to join"),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(Color::srgb(0.45, 0.45, 0.45)),
                    Node {
                        align_self: AlignSelf::Center,
                        margin: UiRect::top(Val::Px(12.0)),
                        ..default()
                    },
                ));
            });

            // ── Settings panel (hidden by default) ──────────────────────────
            root.spawn((
                Name::new("SettingsPanel"),
                SettingsPanel,
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    padding: UiRect::all(Val::Px(48.0)),
                    row_gap: Val::Px(20.0),
                    min_width: Val::Px(420.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.88)),
            ))
            .with_children(|panel| {
                // ── Title ──────────────────────────────────────────────────
                panel.spawn((
                    Text::new("SETTINGS"),
                    TextFont { font_size: 36.0, ..default() },
                    TextColor(Color::srgb(0.85, 0.85, 0.85)),
                    Node {
                        align_self: AlignSelf::Center,
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                ));

                // ── Mouse Sensitivity ───────────────────────────────────────
                panel
                    .spawn((
                        Name::new("SensRow"),
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            width: Val::Percent(100.0),
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new("Mouse Sensitivity"),
                            TextFont { font_size: 18.0, ..default() },
                            TextColor(Color::srgb(0.85, 0.85, 0.85)),
                        ));
                        row.spawn((
                            Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(8.0),
                                ..default()
                            },
                        ))
                        .with_children(|ctrl| {
                            ctrl.spawn((
                                Name::new("SensDecBtn"),
                                Button,
                                SensDecBtn,
                                Node {
                                    width: Val::Px(36.0),
                                    height: Val::Px(36.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.22, 0.22, 0.32)),
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new("<"),
                                    TextFont { font_size: 14.0, ..default() },
                                    TextColor(Color::WHITE),
                                ));
                            });

                            ctrl.spawn((
                                Name::new("SensDisplay"),
                                Text::new(SENSITIVITY_LABELS[settings.sensitivity_idx]),
                                TextFont { font_size: 18.0, ..default() },
                                TextColor(Color::WHITE),
                                Node {
                                    min_width: Val::Px(48.0),
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                                SensDisplay,
                            ));

                            ctrl.spawn((
                                Name::new("SensIncBtn"),
                                Button,
                                SensIncBtn,
                                Node {
                                    width: Val::Px(36.0),
                                    height: Val::Px(36.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.22, 0.22, 0.32)),
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new(">"),
                                    TextFont { font_size: 14.0, ..default() },
                                    TextColor(Color::WHITE),
                                ));
                            });
                        });
                    });

                // ── Anti-Aliasing ───────────────────────────────────────────
                panel
                    .spawn((
                        Name::new("AaRow"),
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            width: Val::Percent(100.0),
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new("Anti-Aliasing"),
                            TextFont { font_size: 18.0, ..default() },
                            TextColor(Color::srgb(0.85, 0.85, 0.85)),
                        ));
                        row.spawn((
                            Name::new("AaCycleBtn"),
                            Button,
                            AaCycleBtn,
                            Node {
                                padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                min_width: Val::Px(110.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.22, 0.22, 0.32)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Name::new("AaDisplay"),
                                Text::new(format!("{} >", settings.msaa.label())),
                                TextFont { font_size: 16.0, ..default() },
                                TextColor(Color::WHITE),
                                AaDisplay,
                            ));
                        });
                    });

                // ── Fullscreen ──────────────────────────────────────────────
                panel
                    .spawn((
                        Name::new("FullscreenRow"),
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            width: Val::Percent(100.0),
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new("Fullscreen"),
                            TextFont { font_size: 18.0, ..default() },
                            TextColor(Color::srgb(0.85, 0.85, 0.85)),
                        ));
                        row.spawn((
                            Name::new("FullscreenToggleBtn"),
                            Button,
                            FullscreenToggleBtn,
                            Node {
                                padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                min_width: Val::Px(110.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.22, 0.22, 0.32)),
                        ))
                        .with_children(|btn| {
                            let label = if settings.fullscreen { "On >" } else { "Off >" };
                            btn.spawn((
                                Name::new("FullscreenDisplay"),
                                Text::new(label),
                                TextFont { font_size: 16.0, ..default() },
                                TextColor(Color::WHITE),
                                FullscreenDisplay,
                            ));
                        });
                    });

                // ── Back button ─────────────────────────────────────────────
                panel
                    .spawn((
                        Name::new("BackFromSettingsButton"),
                        Button,
                        BackFromSettingsButton,
                        Node {
                            align_self: AlignSelf::Center,
                            padding: UiRect::axes(Val::Px(48.0), Val::Px(14.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.20, 0.20, 0.30)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("< Back"),
                            TextFont { font_size: 22.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });
            });
        });
}

fn despawn_connect_screen(
    mut commands: Commands,
    root_query: Query<Entity, With<ConnectScreenRoot>>,
    camera_query: Query<Entity, With<ConnectScreenCamera>>,
) {
    for entity in root_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in camera_query.iter() {
        commands.entity(entity).despawn();
    }
}

// ─── Connecting overlay spawn/despawn ────────────────────────────────────────

fn spawn_connecting_screen(mut commands: Commands, profile: Res<PlayerProfile>) {
    commands.spawn((
        Name::new("ConnectingScreenCamera"),
        ConnectingScreenCamera,
        Camera2d,
    ));

    commands
        .spawn((
            Name::new("ConnectingScreenRoot"),
            ConnectingScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.04, 0.04, 0.10)),
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("ConnectingPanel"),
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(48.0)),
                    row_gap: Val::Px(20.0),
                    min_width: Val::Px(380.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.88)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Connecting..."),
                    TextFont { font_size: 32.0, ..default() },
                    TextColor(Color::WHITE),
                ));
                panel.spawn((
                    Text::new(format!("-> {}", profile.server_addr)),
                    TextFont { font_size: 18.0, ..default() },
                    TextColor(Color::srgb(0.6, 0.6, 0.6)),
                ));
                // Cancel button
                panel
                    .spawn((
                        Name::new("CancelButton"),
                        Button,
                        CancelButton,
                        Node {
                            padding: UiRect::axes(Val::Px(32.0), Val::Px(10.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::top(Val::Px(12.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.40, 0.10, 0.10)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Cancel"),
                            TextFont { font_size: 20.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });
            });
        });
}

fn despawn_connecting_screen(
    mut commands: Commands,
    root_query: Query<Entity, With<ConnectingScreenRoot>>,
    camera_query: Query<Entity, With<ConnectingScreenCamera>>,
) {
    for entity in root_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in camera_query.iter() {
        commands.entity(entity).despawn();
    }
}

// ─── Connect-screen field systems ────────────────────────────────────────────

fn handle_field_click(
    interaction_query: Query<(&Interaction, &InputField), Changed<Interaction>>,
    mut focus: ResMut<FocusedField>,
) {
    for (interaction, field) in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            focus.0 = Some(*field);
        }
    }
}

fn handle_text_input(
    mut key_events: MessageReader<KeyboardInput>,
    focus: Res<FocusedField>,
    mut profile: ResMut<PlayerProfile>,
) {
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        let target = match focus.0 {
            Some(InputField::Username) => &mut profile.username,
            Some(InputField::ServerIp) => &mut profile.server_addr,
            None => continue,
        };
        match &ev.logical_key {
            Key::Character(ch) => {
                target.push_str(ch.as_str());
            }
            Key::Backspace => {
                target.pop();
            }
            _ => {}
        }
    }
}

fn update_field_display(
    profile: Res<PlayerProfile>,
    focus: Res<FocusedField>,
    mut query: Query<(&mut Text, &FieldDisplay)>,
) {
    if !profile.is_changed() && !focus.is_changed() {
        return;
    }
    for (mut text, display) in query.iter_mut() {
        let value = match display.0 {
            InputField::Username => &profile.username,
            InputField::ServerIp => &profile.server_addr,
        };
        let cursor = if focus.0 == Some(display.0) { "|" } else { " " };
        **text = format!("{}{}", value, cursor);
    }
}

/// Highlights the focused input box with a brighter background.
fn update_field_highlight(
    focus: Res<FocusedField>,
    mut query: Query<(&mut BackgroundColor, &InputField)>,
) {
    if !focus.is_changed() {
        return;
    }
    for (mut bg, field) in query.iter_mut() {
        *bg = if focus.0 == Some(*field) {
            BackgroundColor(Color::srgb(0.20, 0.20, 0.30))
        } else {
            BackgroundColor(Color::srgb(0.14, 0.14, 0.20))
        };
    }
}

fn handle_connect_button(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<ConnectButton>)>,
    keys: Res<ButtonInput<KeyCode>>,
    mut profile: ResMut<PlayerProfile>,
    mut next_state: ResMut<NextState<GameState>>,
    mut error_query: Query<&mut Text, With<ErrorText>>,
    mut conn_error: ResMut<ConnectionError>,
) {
    let mut try_connect = keys.just_pressed(KeyCode::Enter);
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            try_connect = true;
        }
    }
    if !try_connect {
        return;
    }

    let Ok(mut error_text) = error_query.single_mut() else {
        return;
    };

    // Trim username.
    let username = profile.username.trim().to_string();
    if username.is_empty() {
        **error_text = "Username is required.".to_string();
        return;
    }
    profile.username = username;

    // Normalise server address: append default port if none provided.
    let addr = profile.server_addr.trim().to_string();
    let addr = if addr.contains(':') {
        addr
    } else {
        format!("{}:7777", addr)
    };
    if addr.is_empty() {
        **error_text = "Server address is required.".to_string();
        return;
    }
    profile.server_addr = addr;

    // Clear any previous connection error and begin the connection attempt.
    conn_error.0 = None;
    **error_text = String::new();
    next_state.set(GameState::Connecting);
}

fn handle_cancel_button(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<CancelButton>)>,
    mut next_state: ResMut<NextState<GameState>>,
    mut conn_error: ResMut<ConnectionError>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            conn_error.0 = Some("Connection cancelled.".to_string());
            next_state.set(GameState::ConnectScreen);
        }
    }
}

// ─── Settings panel systems ───────────────────────────────────────────────────

fn handle_settings_button(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<SettingsButton>)>,
    mut main_node: Query<&mut Node, (With<ConnectMainPanel>, Without<SettingsPanel>)>,
    mut settings_node: Query<&mut Node, (With<SettingsPanel>, Without<ConnectMainPanel>)>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            for mut node in main_node.iter_mut() {
                node.display = Display::None;
            }
            for mut node in settings_node.iter_mut() {
                node.display = Display::Flex;
            }
        }
    }
}

fn handle_back_button(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<BackFromSettingsButton>)>,
    mut main_node: Query<&mut Node, (With<ConnectMainPanel>, Without<SettingsPanel>)>,
    mut settings_node: Query<&mut Node, (With<SettingsPanel>, Without<ConnectMainPanel>)>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            for mut node in main_node.iter_mut() {
                node.display = Display::Flex;
            }
            for mut node in settings_node.iter_mut() {
                node.display = Display::None;
            }
        }
    }
}

fn handle_sens_dec(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<SensDecBtn>)>,
    mut settings: ResMut<GameSettings>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed && settings.sensitivity_idx > 0 {
            settings.sensitivity_idx -= 1;
        }
    }
}

fn handle_sens_inc(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<SensIncBtn>)>,
    mut settings: ResMut<GameSettings>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed
            && settings.sensitivity_idx < SENSITIVITY_STEPS.len() - 1
        {
            settings.sensitivity_idx += 1;
        }
    }
}

fn handle_aa_cycle(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<AaCycleBtn>)>,
    mut settings: ResMut<GameSettings>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            settings.msaa = settings.msaa.next();
        }
    }
}

fn handle_fullscreen_toggle(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<FullscreenToggleBtn>)>,
    mut settings: ResMut<GameSettings>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            settings.fullscreen = !settings.fullscreen;
        }
    }
}

/// Refreshes the three settings display texts whenever GameSettings changes.
fn update_settings_displays(
    settings: Res<GameSettings>,
    mut sens_q: Query<
        &mut Text,
        (With<SensDisplay>, Without<AaDisplay>, Without<FullscreenDisplay>),
    >,
    mut aa_q: Query<
        &mut Text,
        (With<AaDisplay>, Without<SensDisplay>, Without<FullscreenDisplay>),
    >,
    mut fs_q: Query<
        &mut Text,
        (With<FullscreenDisplay>, Without<SensDisplay>, Without<AaDisplay>),
    >,
) {
    if !settings.is_changed() {
        return;
    }
    for mut text in sens_q.iter_mut() {
        **text = SENSITIVITY_LABELS[settings.sensitivity_idx].to_string();
    }
    for mut text in aa_q.iter_mut() {
        **text = format!("{} >", settings.msaa.label());
    }
    for mut text in fs_q.iter_mut() {
        **text = if settings.fullscreen {
            "On >".to_string()
        } else {
            "Off >".to_string()
        };
    }
}
