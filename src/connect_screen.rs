use crate::game::{ConnectionError, GameState, PlayerProfile};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;

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
            root.spawn((
                Name::new("Panel"),
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
                // ── Title ──────────────────────────────────────────────────────
                panel.spawn((
                    Text::new("RUST SHOOTER"),
                    TextFont {
                        font_size: FontSize::Px(46.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.30, 0.10)),
                    Node {
                        align_self: AlignSelf::Center,
                        margin: UiRect::bottom(Val::Px(24.0)),
                        ..default()
                    },
                ));

                // ── Username ───────────────────────────────────────────────────
                panel.spawn((
                    Text::new("Username"),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
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
                            TextFont {
                                font_size: FontSize::Px(20.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            FieldDisplay(InputField::Username),
                        ));
                    });

                // ── Server address (hidden when RSG_SERVER_ADDR env var is set) ─
                if !profile.server_addr_locked {
                    panel.spawn((
                        Text::new("Server Address"),
                        TextFont {
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.72, 0.72, 0.72)),
                        Node {
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
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
                                TextFont {
                                    font_size: FontSize::Px(20.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                FieldDisplay(InputField::ServerIp),
                            ));
                        });
                }

                // ── Error line ─────────────────────────────────────────────────
                let error_msg = conn_error.0.clone().unwrap_or_default();
                panel.spawn((
                    Name::new("ErrorText"),
                    Text::new(error_msg),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.30, 0.30)),
                    Node {
                        min_height: Val::Px(18.0),
                        ..default()
                    },
                    ErrorText,
                ));

                // ── Connect button ─────────────────────────────────────────────
                panel
                    .spawn((
                        Name::new("ConnectButton"),
                        Button,
                        ConnectButton,
                        Node {
                            align_self: AlignSelf::Center,
                            padding: UiRect::axes(Val::Px(48.0), Val::Px(14.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.08, 0.55, 0.12)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("CONNECT"),
                            TextFont {
                                font_size: FontSize::Px(24.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                // ── Hint ───────────────────────────────────────────────────────
                panel.spawn((
                    Text::new("Click a field to type  |  Enter or click CONNECT to join"),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.45, 0.45, 0.45)),
                    Node {
                        align_self: AlignSelf::Center,
                        margin: UiRect::top(Val::Px(12.0)),
                        ..default()
                    },
                ));
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
                    TextFont {
                        font_size: FontSize::Px(32.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                panel.spawn((
                    Text::new(format!("> {}", profile.server_addr)),
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
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
                            TextFont {
                                font_size: FontSize::Px(20.0),
                                ..default()
                            },
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

// ─── Systems ─────────────────────────────────────────────────────────────────

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
