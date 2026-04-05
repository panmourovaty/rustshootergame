use avian3d::prelude::*;
use bevy::prelude::*;

const GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);

fn setup_physics() -> impl System<Startup> {
    move || {
        println!("Setting up physics...");
    }
}

pub struct PhysicsPlugin {
    pub gravity: Vec3,
}

impl Default for PhysicsPlugin {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
        }
    }
}

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Gravity(self.gravity));
    }
}

pub fn setup_physics_world() {
    println!("Physics world initialized");
}
