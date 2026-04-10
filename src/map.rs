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
    }
}
