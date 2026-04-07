use bevy::prelude::*;
use std::collections::HashMap;

pub struct GamePlugin;

// ─── States ─────────────────────────────────────────────────────────────────

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    Playing,
    GameOver,
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
    pub kills: HashMap<u32, u32>,
    pub deaths: HashMap<u32, u32>,
}

impl Scores {
    pub fn add_kill(&mut self, player_id: u32) {
        *self.kills.entry(player_id).or_insert(0) += 1;
    }

    pub fn add_death(&mut self, player_id: u32) {
        *self.deaths.entry(player_id).or_insert(0) += 1;
    }

    pub fn get_kills(&self, player_id: u32) -> u32 {
        *self.kills.get(&player_id).unwrap_or(&0)
    }

    pub fn get_deaths(&self, player_id: u32) -> u32 {
        *self.deaths.get(&player_id).unwrap_or(&0)
    }
}

// ─── Events ─────────────────────────────────────────────────────────────────

/// Emitted when a kill is confirmed, so UI and score tracking can react.
#[derive(Event, Clone, Debug)]
pub struct KillEvent {
    pub killer_id: u32,
    pub victim_id: u32,
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>();
        app.init_resource::<GameConfig>();
        app.init_resource::<Scores>();
        app.add_event::<KillEvent>();

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

pub fn record_kills(mut scores: ResMut<Scores>, mut kill_events: EventReader<KillEvent>) {
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
