use super::*;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Gun {
    pub cooldown: f32,
    pub fire_rate: f32,
}

impl Default for Gun {
    fn default() -> Self {
        Self {
            cooldown: 0.0,
            fire_rate: 0.2,
        }
    }
}

#[derive(Component)]
pub struct Target;

#[derive(Component)]
pub struct Bullet {
    pub lifetime: f32,
}
