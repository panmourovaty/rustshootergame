use bevy::prelude::*;
use avian3d::prelude::*;

pub struct MapPlugin;

// ─── Components ─────────────────────────────────────────────────────────────

/// Marker for every entity that belongs to the built-in placeholder map
/// (physics bodies, meshes, lights).  The dynamic map loader despawns all
/// entities carrying this component before spawning the downloaded map.
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
        // Physics always runs (server + client).
        app.add_systems(Startup, spawn_map_physics);
        // Visuals only run when the full render stack is present (client only).
        // We guard on Assets<StandardMaterial> because the server initialises
        // Assets<Mesh> for avian3d but never registers StandardMaterial.
        app.add_systems(
            Startup,
            spawn_map_visuals.run_if(resource_exists::<Assets<StandardMaterial>>),
        );
    }
}

// ─── Systems ────────────────────────────────────────────────────────────────

/// Spawns invisible collision geometry.  Runs on both the server and the client.
fn spawn_map_physics(mut commands: Commands) {
    // Floor
    commands.spawn((
        Name::new("Floor"),
        HardcodedMap,
        Transform::from_xyz(0.0, -0.5, 0.0),
        RigidBody::Static,
        Collider::cuboid(40.0, 1.0, 40.0),
    ));
    // Ceiling
    commands.spawn((
        Name::new("Ceiling"),
        HardcodedMap,
        Transform::from_xyz(0.0, 4.5, 0.0),
        RigidBody::Static,
        Collider::cuboid(40.0, 1.0, 40.0),
    ));
    // Walls
    for (name, pos, half) in [
        ("Wall_North", Vec3::new(0.0, 2.0, -20.0), (40.0_f32, 5.0_f32, 1.0_f32)),
        ("Wall_South", Vec3::new(0.0, 2.0,  20.0), (40.0, 5.0, 1.0)),
        ("Wall_West",  Vec3::new(-20.0, 2.0, 0.0), (1.0, 5.0, 40.0)),
        ("Wall_East",  Vec3::new( 20.0, 2.0, 0.0), (1.0, 5.0, 40.0)),
    ] {
        commands.spawn((
            Name::new(name),
            HardcodedMap,
            Transform::from_translation(pos),
            RigidBody::Static,
            Collider::cuboid(half.0, half.1, half.2),
        ));
    }
    // Cover boxes
    for pos in [
        Vec3::new(-5.0, 0.75, -5.0),
        Vec3::new( 5.0, 0.75,  5.0),
        Vec3::new(-5.0, 0.75,  5.0),
        Vec3::new( 5.0, 0.75, -5.0),
        Vec3::new( 0.0, 0.75, -9.0),
        Vec3::new( 0.0, 0.75,  9.0),
        Vec3::new(-10.0, 0.75, 0.0),
        Vec3::new( 10.0, 0.75, 0.0),
    ] {
        commands.spawn((
            Name::new("Cover"),
            HardcodedMap,
            Transform::from_translation(pos),
            RigidBody::Static,
            Collider::cuboid(1.5, 1.5, 1.5),
        ));
    }
}

/// Spawns visual meshes, materials and lighting.  Skipped on the server
/// (MinimalPlugins does not register Assets<Mesh>).
fn spawn_map_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let grey_floor = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.25, 0.25),
        perceptual_roughness: 0.9,
        ..default()
    });
    let grey_ceil = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.75, 0.75),
        perceptual_roughness: 0.8,
        ..default()
    });
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.52),
        perceptual_roughness: 0.85,
        ..default()
    });

    // Floor
    commands.spawn((
        Name::new("Floor_Mesh"),
        HardcodedMap,
        Mesh3d(meshes.add(Cuboid::new(40.0, 1.0, 40.0))),
        MeshMaterial3d(grey_floor),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    // Ceiling
    commands.spawn((
        Name::new("Ceiling_Mesh"),
        HardcodedMap,
        Mesh3d(meshes.add(Cuboid::new(40.0, 1.0, 40.0))),
        MeshMaterial3d(grey_ceil),
        Transform::from_xyz(0.0, 4.5, 0.0),
    ));
    // Walls
    for (name, pos, size) in [
        ("Wall_North_Mesh", Vec3::new(0.0, 2.0, -20.0), (40.0_f32, 5.0_f32, 1.0_f32)),
        ("Wall_South_Mesh", Vec3::new(0.0, 2.0,  20.0), (40.0, 5.0, 1.0)),
        ("Wall_West_Mesh",  Vec3::new(-20.0, 2.0, 0.0), (1.0, 5.0, 40.0)),
        ("Wall_East_Mesh",  Vec3::new( 20.0, 2.0, 0.0), (1.0, 5.0, 40.0)),
    ] {
        commands.spawn((
            Name::new(name),
            HardcodedMap,
            Mesh3d(meshes.add(Cuboid::new(size.0, size.1, size.2))),
            MeshMaterial3d(wall_mat.clone()),
            Transform::from_translation(pos),
        ));
    }
    // Cover boxes
    for (pos, color) in [
        (Vec3::new(-5.0, 0.75, -5.0), Color::srgb(0.8, 0.2, 0.2)),
        (Vec3::new( 5.0, 0.75,  5.0), Color::srgb(0.2, 0.3, 0.8)),
        (Vec3::new(-5.0, 0.75,  5.0), Color::srgb(0.2, 0.75, 0.2)),
        (Vec3::new( 5.0, 0.75, -5.0), Color::srgb(0.8, 0.75, 0.1)),
        (Vec3::new( 0.0, 0.75, -9.0), Color::srgb(0.8, 0.4, 0.1)),
        (Vec3::new( 0.0, 0.75,  9.0), Color::srgb(0.5, 0.1, 0.8)),
        (Vec3::new(-10.0, 0.75, 0.0), Color::srgb(0.1, 0.6, 0.7)),
        (Vec3::new( 10.0, 0.75, 0.0), Color::srgb(0.7, 0.1, 0.5)),
    ] {
        commands.spawn((
            Name::new("Cover_Mesh"),
            HardcodedMap,
            Mesh3d(meshes.add(Cuboid::new(1.5, 1.5, 1.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness: 0.7,
                ..default()
            })),
            Transform::from_translation(pos),
        ));
    }
    // Lighting
    commands.spawn((
        Name::new("Sun"),
        HardcodedMap,
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
