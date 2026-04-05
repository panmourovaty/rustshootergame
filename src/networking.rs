use super::*;
use lightyear::prelude::*;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Lightyear::new(ServerConfig::default()));
        app.add_plugins(ClientPlugin::new());
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct NetworkedPlayer {
    pub id: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
}

impl Default for NetworkedPlayer {
    fn default() -> Self {
        Self {
            id: 0,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            rotation: Quat::ONE,
        }
    }
}

#[derive(Events, Component, Debug, Clone)]
pub struct PlayerAction {
    pub move_direction: Vec3,
    pub is_shooting: bool,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for PlayerAction {
    fn default() -> Self {
        Self {
            move_direction: Vec3::ZERO,
            is_shooting: false,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}
