use bevy::prelude::*;
use std::collections::HashMap;

pub struct GamePlugin;

// ─── States ─────────────────────────────────────────────────────────────────

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    /// Initial state — client shows connect screen; server skips straight to Loading.
    #[default]
    ConnectScreen,
    /// Network handshake in progress; transitions to Playing on success or back
    /// to ConnectScreen on timeout/cancel.
    Connecting,
    /// Brief transitional state; immediately advances to Playing.
    Loading,
    Playing,
    GameOver,
}

// ─── Connection error ────────────────────────────────────────────────────────

/// Populated by the network plugin when a connection attempt fails.
/// The connect screen reads this on re-entry and shows the message.
#[derive(Resource, Default)]
pub struct ConnectionError(pub Option<String>);

// ─── Player profile ──────────────────────────────────────────────────────────

/// Set by the connect screen before transitioning to Playing.
#[derive(Resource)]
pub struct PlayerProfile {
    pub username: String,
    /// Full socket address, e.g. "127.0.0.1:7777".
    pub server_addr: String,
    /// True when `RSG_SERVER_ADDR` env var was set at launch; hides the IP field
    /// on the connect screen so users cannot change it.
    pub server_addr_locked: bool,
    /// Stable random ID generated once at startup and used as the network identity.
    pub client_id: u64,
}

impl Default for PlayerProfile {
    fn default() -> Self {
        let (server_addr, server_addr_locked) = resolve_server_addr();
        Self {
            username: String::new(),
            server_addr,
            server_addr_locked,
            client_id: generate_client_id(),
        }
    }
}

fn generate_client_id() -> u64 {
    let mut buf = [0u8; 8];
    getrandom::getrandom(&mut buf).expect("getrandom failed");
    u64::from_le_bytes(buf)
}

/// Native: reads `RSG_SERVER_ADDR` environment variable.
#[cfg(not(target_arch = "wasm32"))]
fn resolve_server_addr() -> (String, bool) {
    match std::env::var("RSG_SERVER_ADDR") {
        Ok(val) if !val.is_empty() => (val, true),
        _ => ("127.0.0.1:7777".to_string(), false),
    }
}

/// WASM: reads `window.__RSG_SERVER_ADDR__` injected by index.html from server.txt.
/// Falls back to showing the address field if the file was absent or empty.
#[cfg(target_arch = "wasm32")]
fn resolve_server_addr() -> (String, bool) {
    let addr = js_sys::Reflect::get(
        &js_sys::global(),
        &wasm_bindgen::JsValue::from_str("__RSG_SERVER_ADDR__"),
    )
    .ok()
    .and_then(|v| v.as_string())
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty());

    match addr {
        Some(a) => (a, true),
        None => ("127.0.0.1:7778".to_string(), false),
    }
}

// ─── Resources ──────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct GameConfig {
    pub kill_limit: u32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self { kill_limit: 10 }
    }
}

#[derive(Resource, Default)]
pub struct Scores {
    pub kills: HashMap<u64, u32>,
    pub deaths: HashMap<u64, u32>,
}

impl Scores {
    pub fn add_kill(&mut self, player_id: u64) {
        *self.kills.entry(player_id).or_insert(0) += 1;
    }

    pub fn add_death(&mut self, player_id: u64) {
        *self.deaths.entry(player_id).or_insert(0) += 1;
    }

    pub fn get_kills(&self, player_id: u64) -> u32 {
        *self.kills.get(&player_id).unwrap_or(&0)
    }

    pub fn get_deaths(&self, player_id: u64) -> u32 {
        *self.deaths.get(&player_id).unwrap_or(&0)
    }
}

/// Maps network client_id → display username.
#[derive(Resource, Default)]
pub struct PlayerNames(pub HashMap<u64, String>);

// ─── Events ─────────────────────────────────────────────────────────────────

/// Emitted when a kill is confirmed, so UI and score tracking can react.
#[derive(Message, Clone, Debug)]
pub struct KillEvent {
    pub killer_id: u64,
    pub victim_id: u64,
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>();
        app.init_resource::<GameConfig>();
        app.init_resource::<Scores>();
        app.init_resource::<PlayerProfile>();
        app.init_resource::<ConnectionError>();
        app.init_resource::<PlayerNames>();
        app.add_message::<KillEvent>();

        app.add_systems(Startup, setup_lighting);

        // Immediately transition out of Loading on entry.
        app.add_systems(
            OnEnter(GameState::Loading),
            |mut next: ResMut<NextState<GameState>>| {
                next.set(GameState::Playing);
            },
        );

        app.add_systems(
            Update,
            record_kills.run_if(in_state(GameState::Playing)),
        );
        app.add_systems(
            Update,
            check_win_condition.run_if(in_state(GameState::Playing)),
        );
    }
}

// ─── Systems ────────────────────────────────────────────────────────────────

/// Spawns a warm directional sun and a dim sky-blue ambient light.
///
/// The sun is positioned 100 m above the origin and angled ≈ 30° off vertical
/// (toward negative X/Z) so shadows always have a clear direction rather than
/// pointing straight down.
fn setup_lighting(mut commands: Commands, mut ambient: ResMut<AmbientLight>) {
    // Sky-blue ambient fill — keeps shadowed areas from going pitch-black.
    ambient.color = Color::srgb(0.55, 0.65, 0.85);
    ambient.brightness = 400.0;

    // Warm sun: from (0, 100, 0) aimed at (-30, 0, -30) ≈ 30° from zenith.
    commands.spawn((
        Name::new("Sun"),
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.82),
            illuminance: 25_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 100.0, 0.0)
            .looking_at(Vec3::new(-30.0, 0.0, -30.0), Vec3::Z),
    ));
}

pub fn record_kills(mut scores: ResMut<Scores>, mut kill_events: MessageReader<KillEvent>) {
    for ev in kill_events.read() {
        scores.add_kill(ev.killer_id);
        scores.add_death(ev.victim_id);
        info!(
            "Kill registered: player {} killed player {} ({} total kills)",
            ev.killer_id,
            ev.victim_id,
            scores.get_kills(ev.killer_id)
        );
    }
}

fn check_win_condition(
    scores: Res<Scores>,
    config: Res<GameConfig>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for (_, &kills) in &scores.kills {
        if kills >= config.kill_limit {
            next_state.set(GameState::GameOver);
            info!("Game Over! A player reached {} kills.", config.kill_limit);
        }
    }
}
