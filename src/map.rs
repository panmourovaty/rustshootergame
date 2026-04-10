use avian3d::prelude::*;
use bevy::prelude::*;

pub struct MapPlugin;

// ─── Components ─────────────────────────────────────────────────────────────

/// Marker for every entity that belongs to the built-in placeholder map.
/// The dynamic map loader despawns all entities carrying this component
/// before spawning the downloaded map.
#[derive(Component)]
pub struct HardcodedMap;

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
        app.add_systems(Startup, spawn_hardcoded_map);
    }
}

// ─── Systems ────────────────────────────────────────────────────────────────

/// Spawns a simple placeholder arena: a 50×50 m floor and four perimeter walls.
/// All entities are tagged with `HardcodedMap` so the dynamic map loader can
/// despawn them when a server-provided map is downloaded.
fn spawn_hardcoded_map(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Floor: 50×50 m slab, top surface at y = 0.
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.35, 0.35),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        Name::new("HardcodedFloor"),
        HardcodedMap,
        Mesh3d(meshes.add(Cuboid::new(50.0, 0.5, 50.0))),
        MeshMaterial3d(floor_mat),
        Transform::from_xyz(0.0, -0.25, 0.0),
        RigidBody::Static,
        Collider::cuboid(25.0, 0.25, 25.0),
    ));

    // Four perimeter walls (50 m long, 5 m tall, 0.5 m thick).
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.28, 0.35),
        perceptual_roughness: 0.8,
        ..default()
    });
    for (pos, hx, hy, hz) in [
        // North (z = −25)
        (Vec3::new(  0.0, 2.5, -25.25_f32), 25.0_f32, 2.5_f32, 0.25_f32),
        // South (z = +25)
        (Vec3::new(  0.0, 2.5,  25.25), 25.0, 2.5, 0.25),
        // West  (x = −25)
        (Vec3::new(-25.25, 2.5,   0.0), 0.25, 2.5, 25.0),
        // East  (x = +25)
        (Vec3::new( 25.25, 2.5,   0.0), 0.25, 2.5, 25.0),
    ] {
        commands.spawn((
            Name::new("HardcodedWall"),
            HardcodedMap,
            Mesh3d(meshes.add(Cuboid::new(hx * 2.0, hy * 2.0, hz * 2.0))),
            MeshMaterial3d(wall_mat.clone()),
            Transform::from_translation(pos),
            RigidBody::Static,
            Collider::cuboid(hx, hy, hz),
        ));
    }
}
