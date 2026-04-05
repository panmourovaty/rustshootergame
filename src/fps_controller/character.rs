use super::*;

#[derive(Component)]
pub struct Character {
    pub velocity: Vec3,
}

impl Default for Character {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
        }
    }
}

#[derive(Component)]
pub struct CharacterController;

pub fn character_controller(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut characters: Query<(&Transform, &mut Character, &CharacterController)>,
) {
    for (transform, mut character, _controller) in characters.iter_mut() {
        let mut speed = 5.0;
        let mut direction = Vec3::ZERO;

        if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
            direction -= transform.forward();
        }
        if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
            direction += transform.forward();
        }
        if keyboard.pressed(KeyCode::KeyA) {
            direction -= transform.right();
        }
        if keyboard.pressed(KeyCode::KeyD) {
            direction += transform.right();
        }

        direction.y = 0.0;
        direction.normalize_or_set_length(0.0);

        if direction.length_squared() > 0.0 {
            character.velocity.x = direction.x * speed;
            character.velocity.z = direction.z * speed;
        }

        character.velocity.x *= 0.9;
        character.velocity.z *= 0.9;

        transform.translation.x += character.velocity.x * time.delta_secs();
        transform.translation.z += character.velocity.z * time.delta_secs();
    }
}
