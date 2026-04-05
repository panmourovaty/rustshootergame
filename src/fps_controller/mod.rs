pub mod camera;

#[derive(Component)]
pub struct LogicalPlayer;

#[derive(Component)]
pub struct RenderPlayer {
    pub logical_entity: Entity,
}

#[derive(Component)]
pub struct CameraConfig {
    pub height_offset: f32,
}

#[derive(Component, Default)]
pub struct FpsControllerInput {
    pub fly: bool,
    pub sprint: bool,
    pub jump: bool,
    pub crouch: bool,
    pub pitch: f32,
    pub yaw: f32,
    pub movement: Vec3,
}

#[derive(Component)]
pub struct FpsController {
    pub radius: f32,
    pub height: f32,
    pub upright_height: f32,
    pub crouch_height: f32,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub forward_speed: f32,
    pub side_speed: f32,
    pub acceleration: f32,
    pub friction: f32,
    pub gravity: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub ground_tick: u8,
    pub sensitivity: f32,
    pub enable_input: bool,
    pub previous_translation: Option<Vec3>,
}

impl Default for FpsController {
    fn default() -> Self {
        Self {
            radius: 0.5,
            height: 3.0,
            upright_height: 3.0,
            crouch_height: 1.5,
            walk_speed: 9.0,
            run_speed: 14.0,
            forward_speed: 30.0,
            side_speed: 30.0,
            acceleration: 10.0,
            friction: 10.0,
            gravity: 23.0,
            pitch: 0.0,
            yaw: 0.0,
            ground_tick: 0,
            sensitivity: 0.002,
            enable_input: true,
            previous_translation: None,
        }
    }
}

pub fn insert_fully_keyed_fps_controller(mut commands: Commands) {
    commands.spawn((
        LogicalPlayer,
        FpsController::default(),
        Collider::cylinder(0.5, 3.0),
        Transform::from_translation(Vec3::new(0.0, 10.0, 0.0)),
        CameraConfig {
            height_offset: 2.6,
        },
    ));

    commands.spawn((
        RenderPlayer {
            logical_entity: Entity::PLACEHOLDER,
        },
        Transform::from_translation(Vec3::new(0.0, 12.0, 0.0)),
    ));
}
