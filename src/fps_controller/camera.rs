use super::*;
use bevy_fov::TargetCamera;
use bevy_fov::TargetCameraConfig;

pub fn camera_look(
    time: Res<Time>,
    kb: Res<ButtonInput<KeyCode>>,
    mouse: Res<MouseMotion>,
    mut targets: Query<(&mut TargetCamera)>,
) {
    for mut target in targets.iter_mut() {
        let dt = time.delta_secs();

        let sensitivity = if kb.pressed(KeyCode::LShift) {
            target.config.sensitivity * 5.0
        } else {
            target.config.sensitivity
        };

        let pitch = *target.pitch_mut();
        let yaw = *target.yaw_mut();

        let delta_pitch = mouse.y * -1. * sensitivity * dt;
        let delta_yaw = mouse.x * -1. * sensitivity * dt;

        let new_pitch = pitch.saturating_add(delta_pitch).clamp(-target.config.max_pitch, target.config.max_pitch);

        *target.pitch_mut() = new_pitch;
        *target.yaw_mut() = yaw + delta_yaw;
    }
}

pub fn look_transforms(mut query: Query<(&TargetCamera, &mut Transform)>) {
    for (camera, mut transform) in query.iter_mut() {
        let pitch = *camera.pitch();
        let yaw = *camera.yaw();
        let roll = 0.0;
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    }
}

pub fn camera_transparency_on_click(
    mut camera_query: Query<&mut Camera, (With<Camera3d>, Without<TargetCamera>)>,
) {
    for mut camera in camera_query.iter_mut() {
        camera.clear_color = ClearColorConfig::Custom(Color::rgba(0.14, 0.14, 0.14, 0.05));
    }
}
