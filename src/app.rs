use super::*;
use bevy_fov::TargetCamera;
use crate::fps_controller::*;
use crate::components::{Player, Gun, Target};
use crate::shooting::*;

pub fn setup() -> Startup<'static> {
    Startup::default()
        .init_resource::<TargetCamera>()
        .add_systems(Startup, (
            setup_lighting,
            setup_ground,
            setup_player,
            setup_test_objects,
            insert_fully_keyed_fps_controller,
        ))
}

pub fn setup_game() -> Update<'static> {
    Update::default()
        .add_systems(Update, (
            crate::shooting::shoot,
        ))
}

fn setup_lighting(mut commands: Commands) {
    commands.spawn((
        AmbientLight::new(Color::srgb(0.5, 0.5, 0.5)),
        PointLight {
            intensity: 15000.0,
            color: Color::WHITE,
            range: 1000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 1500.0, 0.0)),
    ));
}

fn setup_ground(mut commands: Commands) {
    commands.spawn((
        Mesh3d(Cuboid {
            size: Vec3::new(2000.0, 1.0, 2000.0),
        }),
        Material3d(colors::BLUE),
        Transform::from_translation(Vec3::new(0.0, -0.5, 0.0)),
    ));
}

fn setup_player(mut commands: Commands) {
    commands.spawn((
        Player::default(),
        Gun::default(),
        Transform::from_translation(Vec3::new(0.0, 10.0, 0.0)),
    ));
}

fn setup_test_objects(mut commands: Commands) {
    for x in -5..=5 {
        for z in -5..=5 {
            if x == 0 && z == 0 {
                continue;
            }
            commands.spawn((
                Target::default(),
                Mesh3d(Cuboid {
                    size: Vec3::new(10.0, 20.0, 10.0),
                }),
                Material3d(Color::srgb(x as f32 * 0.1, z as f32 * 0.1, 0.5)),
                Transform::from_translation(Vec3::new(
                    x as f32 * 30.0,
                    10.0,
                    z as f32 * 30.0,
                )),
            ));
        }
    }
}
