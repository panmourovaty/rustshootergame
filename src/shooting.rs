use super::*;
use crate::components::{Bullet, Gun, Target};

const BULLET_SPEED: f32 = 50.0;
const BULLET_LIFETIME: f32 = 2.0;

fn spawn_bullet(
    mut commands: Commands,
    player_query: Query<(Entity, &Transform), With<Gun>>,
) {
    for (entity, transform) in player_query.iter() {
        let bullet_rotation = transform.rotation;
        let bullet_velocity = Vec3::Y * BULLET_SPEED;

        commands.spawn((
            Bullet {
                lifetime: BULLET_LIFETIME,
            },
            Mesh3d(Cuboid {
                size: Vec3::new(1.0, 1.0, 1.0),
            }),
            Material3d(Color::BLACK),
            Transform::from_translation(transform.translation)
                .looking_at(transform.translation + bullet_velocity, Vec3::Y),
        ));
    }
}

fn bullet_fall(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Bullet)>,
) {
    for (mut transform, mut bullet) in query.iter_mut() {
        bullet.lifetime -= time.delta_secs();

        if bullet.lifetime <= 0.0 {
            bullet.lifetime = 0.0;
            continue;
        }

        transform.translation.y -= time.delta_secs() * 0.3;
    }
}

fn bullet_collision(
    mut commands: Commands,
    bullets: Query<(Entity, &Transform), With<Bullet>>,
    mut targets: Query<(Entity, &mut Transform), With<Target>>,
) {
    for (bullet_entity, bullet_transform) in bullets.iter() {
        for (target_entity, mut target_transform) in targets.iter_mut() {
            let distance = (bullet_transform.translation - target_transform.translation).length();
            
            if distance < 10.0 {
                commands.entity(bullet_entity).despawn();
                target_transform.translation += Vec3::Y * 0.1;
                break;
            }
        }
    }
}

pub fn shoot(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut guns: Query<(&mut Gun, &Transform)>,
    mut commands: Commands,
) {
    for (mut gun, transform) in guns.iter_mut() {
        gun.cooldown -= time.delta_secs();

        if gun.cooldown <= 0.0 && keyboard.just_pressed(KeyCode::LeftCtrl) {
            commands.spawn((
                Bullet {
                    lifetime: BULLET_LIFETIME,
                },
                Mesh3d(Cuboid {
                    size: Vec3::new(0.5, 0.5, 0.5),
                }),
                Material3d(Color::BLACK),
                Transform::from_translation(transform.translation + Vec3::new(0.5, -0.5, 1.0))
                    .looking_at(transform.translation + transform.forward() * 100.0, Vec3::Y),
            ));
            gun.cooldown = gun.fire_rate;
        }
    }
}
