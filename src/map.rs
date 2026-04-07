use bevy::prelude::*;
use avian3d::prelude::*;

pub struct MapPlugin;

// ─── Resources ──────────────────────────────────────────────────────────────

/// Possible spawn locations used by the player and respawn system.
#[derive(Resource)]
pub struct SpawnPoints(pub Vec<Vec3>);

// ─── Plugin ─────────────────────────────────────────────────────────────────

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpawnPoints(vec![
            Vec3::new(-15.0, 2.0, -15.0),
            Vec3::new(15.0, 2.0, 15.0),
            Vec3::new(-15.0, 2.0, 15.0),
            Vec3::new(15.0, 2.0, -15.0),
        ]));
        app.add_systems(Startup, spawn_map);
    }
}

// ─── Systems ────────────────────────────────────────────────────────────────

fn spawn_map(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // ── Floor ────────────────────────────────────────────────────────────────
    // Visual: 40×1×40.  avian3d 0.5 Collider::cuboid takes full extents.
    commands.spawn((
        Name::new("Floor"),
        Mesh3d(meshes.add(Cuboid::new(40.0, 1.0, 40.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.25, 0.25, 0.25),
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.5, 0.0),
        RigidBody::Static,
        Collider::cuboid(40.0, 1.0, 40.0),
    ));

    // ── Ceiling ───────────────────────────────────────────────────────────────
    commands.spawn((
        Name::new("Ceiling"),
        Mesh3d(meshes.add(Cuboid::new(40.0, 1.0, 40.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.75, 0.75, 0.75),
            perceptual_roughness: 0.8,
            ..default()
        })),
        Transform::from_xyz(0.0, 4.5, 0.0),
        RigidBody::Static,
        Collider::cuboid(40.0, 1.0, 40.0),
    ));

    // ── Walls ─────────────────────────────────────────────────────────────────
    // Arena is 40×5×40 (interior).  Each wall is 1 unit thick.
    // avian3d 0.5 Collider::cuboid takes full extents matching mesh dimensions.
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.52),
        perceptual_roughness: 0.85,
        ..default()
    });

    // North wall (−Z face)
    commands.spawn((
        Name::new("Wall_North"),
        Mesh3d(meshes.add(Cuboid::new(40.0, 5.0, 1.0))),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_xyz(0.0, 2.0, -20.0),
        RigidBody::Static,
        Collider::cuboid(40.0, 5.0, 1.0),
    ));

    // South wall (+Z face)
    commands.spawn((
        Name::new("Wall_South"),
        Mesh3d(meshes.add(Cuboid::new(40.0, 5.0, 1.0))),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_xyz(0.0, 2.0, 20.0),
        RigidBody::Static,
        Collider::cuboid(40.0, 5.0, 1.0),
    ));

    // West wall (−X face)
    commands.spawn((
        Name::new("Wall_West"),
        Mesh3d(meshes.add(Cuboid::new(1.0, 5.0, 40.0))),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_xyz(-20.0, 2.0, 0.0),
        RigidBody::Static,
        Collider::cuboid(1.0, 5.0, 40.0),
    ));

    // East wall (+X face)
    commands.spawn((
        Name::new("Wall_East"),
        Mesh3d(meshes.add(Cuboid::new(1.0, 5.0, 40.0))),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_xyz(20.0, 2.0, 0.0),
        RigidBody::Static,
        Collider::cuboid(1.0, 5.0, 40.0),
    ));

    // ── Cover boxes ───────────────────────────────────────────────────────────
    // 1.5×1.5×1.5 cubes; full extents passed to Collider::cuboid (avian3d 0.5).
    let covers: &[(Vec3, Color)] = &[
        (Vec3::new(-5.0, 0.75, -5.0), Color::srgb(0.8, 0.2, 0.2)),  // Red
        (Vec3::new(5.0, 0.75, 5.0), Color::srgb(0.2, 0.3, 0.8)),    // Blue
        (Vec3::new(-5.0, 0.75, 5.0), Color::srgb(0.2, 0.75, 0.2)),  // Green
        (Vec3::new(5.0, 0.75, -5.0), Color::srgb(0.8, 0.75, 0.1)),  // Yellow
        (Vec3::new(0.0, 0.75, -9.0), Color::srgb(0.8, 0.4, 0.1)),   // Orange
        (Vec3::new(0.0, 0.75, 9.0), Color::srgb(0.5, 0.1, 0.8)),    // Purple
        (Vec3::new(-10.0, 0.75, 0.0), Color::srgb(0.1, 0.6, 0.7)),  // Cyan
        (Vec3::new(10.0, 0.75, 0.0), Color::srgb(0.7, 0.1, 0.5)),   // Magenta
    ];

    for (pos, color) in covers {
        commands.spawn((
            Name::new("Cover"),
            Mesh3d(meshes.add(Cuboid::new(1.5, 1.5, 1.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: *color,
                perceptual_roughness: 0.7,
                ..default()
            })),
            Transform::from_translation(*pos),
            RigidBody::Static,
            Collider::cuboid(1.5, 1.5, 1.5),
        ));
    }

    // ── Lighting ──────────────────────────────────────────────────────────────
    commands.spawn((
        Name::new("Sun"),
        DirectionalLight {
            illuminance: 15_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.5, 0.0)),
    ));

    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
        ..default()
    });
}
