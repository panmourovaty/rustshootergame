use bevy::prelude::*;
use bevy::window::CursorOptions;
use avian3d::prelude::*;
use crate::player::{Health, LocalPlayer, Player};
use crate::game::KillEvent;
use crate::pvp::RemotePlayer;

pub struct WeaponPlugin;

// ─── Components ─────────────────────────────────────────────────────────────

/// Pistol weapon state — lives on the logical player entity.
#[derive(Component)]
pub struct Weapon {
    pub ammo: u32,
    pub max_ammo: u32,
    /// Minimum seconds between consecutive shots.
    pub fire_rate: f32,
    pub damage: f32,
    /// `Time::elapsed_secs()` value recorded when the last shot was fired.
    pub last_fire_time: f32,
    pub is_reloading: bool,
    /// Counts down from `reload_duration` → 0.
    pub reload_timer: f32,
    pub reload_duration: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            ammo: 30,
            max_ammo: 30,
            fire_rate: 0.15,
            damage: 25.0,
            last_fire_time: -999.0,
            is_reloading: false,
            reload_timer: 0.0,
            reload_duration: 2.0,
        }
    }
}

/// Transient visual marker for a bullet impact spark.
#[derive(Component)]
pub struct ImpactEffect {
    pub timer: Timer,
}

// ─── Events ──────────────────────────────────────────────────────────────────

/// Fired when the player successfully pulls the trigger.
#[derive(Message, Clone, Debug)]
pub struct ShootEvent {
    pub origin: Vec3,
    pub direction: Dir3,
    pub shooter: Entity,
    pub damage: f32,
}

/// Fired when a raycast hits an entity with a `Health` component.
#[derive(Message, Clone, Debug)]
pub struct HitEvent {
    pub shooter_entity: Entity,
    pub target: Entity,
    pub damage: f32,
    pub hit_point: Vec3,
}

/// Fired when the local player's shot hits a remote player.
/// Read by the network client to send a `HitMsg` to the server.
#[derive(Message, Clone, Debug)]
pub struct RemoteHitEvent {
    pub victim_id: u64,
    pub damage: f32,
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ShootEvent>();
        app.add_message::<HitEvent>();
        app.add_message::<RemoteHitEvent>();
        app.add_systems(
            Update,
            (
                handle_reload,
                handle_shooting,
                process_shoot_events,
                apply_damage,
                update_impacts,
            )
                .chain(),
        );
    }
}

// ─── Systems ────────────────────────────────────────────────────────────────

/// R key starts reload when magazine is not full and not already reloading.
fn handle_reload(
    time: Res<Time>,
    key: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Weapon, With<LocalPlayer>>,
) {
    let Ok(mut weapon) = query.single_mut() else {
        return;
    };

    if key.just_pressed(KeyCode::KeyR)
        && !weapon.is_reloading
        && weapon.ammo < weapon.max_ammo
    {
        weapon.is_reloading = true;
        weapon.reload_timer = weapon.reload_duration;
        info!("Reloading…");
        return;
    }

    if weapon.is_reloading {
        weapon.reload_timer -= time.delta_secs();
        if weapon.reload_timer <= 0.0 {
            weapon.ammo = weapon.max_ammo;
            weapon.is_reloading = false;
            info!("Reload complete.");
        }
    }
}

/// Reads mouse input and emits `ShootEvent` when all conditions are met.
fn handle_shooting(
    time: Res<Time>,
    mouse_btn: Res<ButtonInput<MouseButton>>,
    cursor_query: Query<&CursorOptions>,
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
    mut weapon_query: Query<(Entity, &mut Weapon), With<LocalPlayer>>,
    mut shoot_events: MessageWriter<ShootEvent>,
) {
    // Gameplay is only active while the cursor is locked.
    let Ok(cursor) = cursor_query.single() else {
        return;
    };
    if cursor.visible {
        return;
    }

    let Ok((shooter_entity, mut weapon)) = weapon_query.single_mut() else {
        return;
    };

    if weapon.is_reloading {
        return;
    }

    let elapsed = time.elapsed_secs();

    if mouse_btn.pressed(MouseButton::Left)
        && weapon.ammo > 0
        && (elapsed - weapon.last_fire_time) >= weapon.fire_rate
    {
        weapon.ammo -= 1;
        weapon.last_fire_time = elapsed;

        if let Ok(cam_tf) = camera_query.single() {
            shoot_events.write(ShootEvent {
                origin: cam_tf.translation(),
                direction: cam_tf.forward(),
                shooter: shooter_entity,
                damage: weapon.damage,
            });
        }

        if weapon.ammo == 0 {
            info!("Magazine empty — press R to reload.");
        }
    }
}

/// Performs the raycast for each `ShootEvent` and spawns an impact spark.
/// Also detects hits on RemotePlayer entities and fires `RemoteHitEvent`.
fn process_shoot_events(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shoot_events: MessageReader<ShootEvent>,
    mut hit_events: MessageWriter<HitEvent>,
    mut remote_hit_events: MessageWriter<RemoteHitEvent>,
    spatial_query: SpatialQuery,
    health_query: Query<&Health>,
    remote_player_query: Query<&RemotePlayer>,
) {
    for event in shoot_events.read() {
        // Exclude the shooter from the ray so it cannot hit itself.
        let filter = SpatialQueryFilter {
            excluded_entities: [event.shooter].into_iter().collect(),
            ..default()
        };

        if let Some(hit) = spatial_query.cast_ray(
            event.origin,
            event.direction,
            100.0,
            true,
            &filter,
        ) {
            // In avian3d 0.5, the field is `distance` (not `time_of_impact`).
            let hit_point = event.origin + *event.direction * hit.distance;

            // Check if the hit entity is a remote player.
            if let Ok(remote_player) = remote_player_query.get(hit.entity) {
                remote_hit_events.write(RemoteHitEvent {
                    victim_id: remote_player.client_id,
                    damage: event.damage,
                });
                info!(
                    "Remote player {} hit for {:.0} damage",
                    remote_player.client_id, event.damage
                );
            } else if health_query.get(hit.entity).is_ok() {
                // Local entity with health (e.g. local player if they could
                // somehow shoot themselves — kept for future use).
                hit_events.write(HitEvent {
                    shooter_entity: event.shooter,
                    target: hit.entity,
                    damage: event.damage,
                    hit_point,
                });
            }

            // Visual impact spark.
            commands.spawn((
                Name::new("ImpactEffect"),
                Mesh3d(meshes.add(Sphere::new(0.05))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgba(1.0, 1.0, 0.0, 1.0),
                    emissive: LinearRgba::new(3.0, 3.0, 0.0, 1.0),
                    ..default()
                })),
                Transform::from_translation(hit_point),
                ImpactEffect {
                    timer: Timer::from_seconds(0.15, TimerMode::Once),
                },
            ));
        }
    }
}

/// Applies damage and emits `KillEvent` on kill.
fn apply_damage(
    mut hit_events: MessageReader<HitEvent>,
    mut health_query: Query<(&mut Health, Option<&Player>)>,
    shooter_query: Query<Option<&Player>>,
    mut kill_events: MessageWriter<KillEvent>,
) {
    for event in hit_events.read() {
        if let Ok((mut health, victim_player)) = health_query.get_mut(event.target) {
            health.current -= event.damage;
            info!(
                "Hit! {:.0} damage dealt — {:.0}/{:.0} HP remaining",
                event.damage, health.current, health.max
            );

            if health.current <= 0.0 {
                let killer_id = shooter_query
                    .get(event.shooter_entity)
                    .ok()
                    .flatten()
                    .map(|p| p.id)
                    .unwrap_or(0);
                let victim_id = victim_player.map(|p| p.id).unwrap_or(0);

                info!("Kill! Player {} killed player {}.", killer_id, victim_id);
                kill_events.write(KillEvent { killer_id, victim_id });
            }
        }
    }
}

/// Ticks impact spark timers and despawns finished effects.
fn update_impacts(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ImpactEffect)>,
) {
    for (entity, mut effect) in query.iter_mut() {
        effect.timer.tick(time.delta());
        if effect.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
