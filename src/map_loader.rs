/// Dynamic map loader.
///
/// When the server sends a `MapUrlMsg`, this module:
///   1. Downloads the `.tar.zst` archive from the given HTTPS URL on a
///      background thread (native) or via the browser fetch API (WASM).
///   2. Decompresses and extracts the archive into the in-memory "map://"
///      asset source registered at startup.
///   3. Asks Bevy's `AssetServer` to load `map://scene.glb` (visuals) and
///      `map://collision.glb` (physics).
///   4. Once both GLTF files are loaded, despawns the built-in placeholder
///      map entities (marked with `HardcodedMap`) and spawns the GLTF scenes.
///      `collision.glb` gets a `ColliderConstructorHierarchy` so Avian3D
///      auto-generates trimesh colliders for every mesh in that scene.
///   5. Optionally reads `spawn_points.txt` from the archive (one "x y z"
///      per line) to update the `SpawnPoints` resource.
///
/// # Archive layout expected by the loader
///
/// ```
/// map.tar.zst
/// ├── scene.glb         ← required: visual geometry + lighting
/// ├── collision.glb     ← required: invisible collision meshes
/// ├── textures/         ← optional: external textures referenced by the GLB
/// │   ├── floor_albedo.png
/// │   └── …
/// └── spawn_points.txt  ← optional: spawn positions, one "x y z" per line
/// ```
///
/// See `MAP_FORMAT.md` in the repository root for the full authoring guide.

use bevy::asset::io::{
    memory::{Dir, MemoryAssetReader},
    AssetSourceBuilder,
};
use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::Skybox;
use bevy::gltf::{convert_coordinates::GltfConvertCoordinates, Gltf, GltfLoaderSettings};
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy::render::render_resource::{TextureAspect, TextureViewDescriptor, TextureViewDimension};
use avian3d::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::game::GameState;
use crate::map::{HardcodedMap, SpawnPoints};
use crate::player::{LocalPlayer, PlayerCamera};

// ─── Public surface ──────────────────────────────────────────────────────────

/// The in-memory directory that backs the `map://` asset source.
/// Clone the inner `Dir` to insert files; it is `Arc`-backed so all clones
/// share the same storage.
#[derive(Resource, Clone)]
pub struct MapDir(pub Dir);

/// Fire this message (e.g. from the network client) to trigger a map download.
#[derive(Message, Clone)]
pub struct LoadMapFromUrl(pub String);

pub struct MapLoaderPlugin {
    /// A clone of the `Dir` that was already registered as the `map://`
    /// asset source in `main.rs` before `DefaultPlugins` was added.
    pub dir: Dir,
}

impl Plugin for MapLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MapDir(self.dir.clone()));
        app.add_message::<LoadMapFromUrl>();
        // Show a blocking overlay as soon as the game enters Playing so the
        // player never sees an empty world while waiting for the server map.
        app.add_systems(OnEnter(GameState::Playing), show_waiting_overlay);
        app.add_systems(
            Update,
            (
                handle_load_map_event,
                poll_download,
                poll_gltf_loaded,
                tick_waiting_timeout,
                attach_map_colliders,
                apply_skybox,
            ),
        );
    }
}

/// Convenience function: builds the `AssetSourceBuilder` for the `map://`
/// source and returns both the builder and the shared `Dir`.
/// Call this **before** adding `DefaultPlugins`.
pub fn create_map_asset_source() -> (AssetSourceBuilder, Dir) {
    let dir = Dir::default();
    let dir_for_reader = dir.clone();
    let builder =
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir_for_reader.clone() }));
    (builder, dir)
}

// ─── Internal types ──────────────────────────────────────────────────────────

/// Files extracted from the archive, keyed by their path inside the archive.
struct ExtractedMap {
    files: HashMap<String, Vec<u8>>,
}

/// Resource present while a background download is in progress.
/// The `Option` is `None` until the thread/async-task writes the result.
#[derive(Resource)]
struct PendingDownload(Arc<Mutex<Option<Result<ExtractedMap, String>>>>);

/// Resource present while we wait for the GLTF scene handle to finish loading.
#[derive(Resource)]
struct LoadingMapHandles {
    scene: Handle<Gltf>,
    scene_loaded: bool,
}

/// Holds a fully-configured cubemap `Handle<Image>` extracted from the map
/// archive's `skybox.webp` until it can be attached to the camera.
#[derive(Resource)]
struct PendingSkybox(Handle<Image>);

/// Stored after the map scene entity is spawned.  The `attach_map_colliders`
/// system polls every frame until Bevy's SceneSpawner has instantiated the
/// scene's child entities, then attaches `ColliderConstructorHierarchy`.
///
/// This two-step approach is necessary because avian3d processes
/// `ColliderConstructorHierarchy` in PostUpdate of the same frame it is added,
/// but Bevy's SceneSpawner only creates the GLTF child entities in the *next*
/// frame's PreUpdate — so adding the hierarchy at spawn time means avian3d
/// sees no children, marks the hierarchy done, and never creates any colliders.
#[derive(Resource)]
struct PendingMapCollider(Entity);

/// Marker for entities spawned by the dynamic map so they can be cleaned up
/// when a new map is loaded.
#[derive(Component)]
pub struct DynamicMap;

/// Marker for the full-screen loading overlay shown while a map is being
/// downloaded or its GLTF assets are being loaded.
#[derive(Component)]
struct MapLoadingOverlay;

/// Marks the text node inside the loading overlay so its message can be updated.
#[derive(Component)]
struct MapLoadingLabel;

/// Countdown started when the Playing state is entered.  If a `LoadMapFromUrl`
/// event arrives before it expires the resource is removed; otherwise the
/// overlay is dismissed (server has no custom map configured).
#[derive(Resource)]
struct WaitingForMapTimeout(Timer);

// ─── Systems ─────────────────────────────────────────────────────────────────

/// Spawns a fully-opaque black overlay the moment the Playing state is entered
/// so the player never sees an empty world while waiting for the server to
/// send a map URL.  A 2-second timeout is started; if no map URL arrives by
/// then the overlay is removed (server has no `--map-url` configured).
fn show_waiting_overlay(mut commands: Commands) {
    commands
        .spawn((
            Name::new("MapLoadingOverlay"),
            MapLoadingOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 1.0)),
        ))
        .with_children(|c| {
            c.spawn((
                MapLoadingLabel,
                Text::new("Waiting for server map..."),
                TextFont { font_size: 28.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
    commands.insert_resource(WaitingForMapTimeout(Timer::from_seconds(
        2.0,
        TimerMode::Once,
    )));
}

/// Ticks the waiting-for-map timer.  When it fires, the overlay is removed so
/// the player can at least see the (empty) scene if the server has no map.
fn tick_waiting_timeout(
    time: Res<Time>,
    timeout: Option<ResMut<WaitingForMapTimeout>>,
    mut overlay_query: Query<(Entity, &mut Visibility), With<MapLoadingOverlay>>,
    mut label_query: Query<(Entity, &mut Visibility), With<MapLoadingLabel>>,
    mut commands: Commands,
) {
    let Some(mut timeout) = timeout else { return };
    timeout.0.tick(time.delta());
    if timeout.0.just_finished() {
        hide_and_despawn_overlay(&mut overlay_query, &mut label_query, &mut commands);
        commands.remove_resource::<WaitingForMapTimeout>();
    }
}

/// Reacts to `LoadMapFromUrl`, kicks off a background download, and installs
/// `PendingDownload` so `poll_download` can check on it every frame.
fn handle_load_map_event(
    mut events: MessageReader<LoadMapFromUrl>,
    mut commands: Commands,
    mut label_query: Query<&mut Text, With<MapLoadingLabel>>,
) {
    for event in events.read() {
        let url = event.0.clone();
        info!("[MAP] Starting download: {}", url);

        // Cancel the waiting timeout — a map URL arrived.
        commands.remove_resource::<WaitingForMapTimeout>();

        // Update the overlay that was spawned by show_waiting_overlay.
        for mut text in label_query.iter_mut() {
            **text = "Downloading map...".to_string();
        }

        let slot: Arc<Mutex<Option<Result<ExtractedMap, String>>>> =
            Arc::new(Mutex::new(None));

        #[cfg(not(target_arch = "wasm32"))]
        {
            let slot2 = slot.clone();
            std::thread::spawn(move || {
                let result = native_download_and_extract(&url);
                *slot2.lock().unwrap() = Some(result);
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            let slot2 = slot.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = wasm_download_and_extract(&url).await;
                *slot2.lock().unwrap() = Some(result);
            });
        }

        // Remove any previous pending download / loading state.
        commands.remove_resource::<PendingDownload>();
        commands.remove_resource::<LoadingMapHandles>();
        commands.insert_resource(PendingDownload(slot));
    }
}

/// Checks whether the background download has finished.  On success, inserts
/// all extracted files into the `MapDir` and kicks off GLTF loading.
fn poll_download(
    pending: Option<Res<PendingDownload>>,
    map_dir: Res<MapDir>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut spawn_points: ResMut<SpawnPoints>,
    mut commands: Commands,
    mut overlay_query: Query<(Entity, &mut Visibility), With<MapLoadingOverlay>>,
    mut label_vis_query: Query<(Entity, &mut Visibility), With<MapLoadingLabel>>,
    mut label_query: Query<&mut Text, With<MapLoadingLabel>>,
) {
    let Some(pending) = pending else { return };

    let mut slot = pending.0.lock().unwrap();
    if slot.is_none() {
        return; // still downloading
    }

    let result = slot.take().unwrap();
    commands.remove_resource::<PendingDownload>();

    match result {
        Err(e) => {
            error!("[MAP] Download/extract failed: {}", e);
            // Remove the overlay — don't leave a black screen on failure.
            hide_and_despawn_overlay(&mut overlay_query, &mut label_vis_query, &mut commands);
        }
        Ok(extracted) => {
            info!("[MAP] Extracted {} files; inserting into map:// source", extracted.files.len());

            // Advance the overlay message — download done, now waiting for GPU upload.
            for mut text in label_query.iter_mut() {
                **text = "Loading map assets...".to_string();
            }

            // Populate spawn points from the optional text file.
            if let Some(sp_bytes) = extracted.files.get("spawn_points.txt") {
                let text = String::from_utf8_lossy(sp_bytes);
                let points: Vec<Vec3> = text
                    .lines()
                    .filter_map(parse_vec3_line)
                    .collect();
                if !points.is_empty() {
                    info!("[MAP] Loaded {} spawn points", points.len());
                    spawn_points.0 = points;
                }
            }

            // Build cubemap from skybox.webp if present.
            // The WEBP must be a vertical strip of 6 square faces (width W,
            // height 6W) in order: +X, -X, +Y, -Y, +Z, -Z.
            if let Some(skybox_bytes) = extracted.files.get("skybox.webp") {
                match Image::from_buffer(
                    skybox_bytes,
                    ImageType::Extension("webp"),
                    CompressedImageFormats::NONE,
                    true,
                    ImageSampler::Default,
                    RenderAssetUsages::RENDER_WORLD,
                ) {
                    Ok(mut img) => {
                        match img.reinterpret_stacked_2d_as_array(6) {
                            Ok(()) => {
                                img.texture_view_descriptor = Some(TextureViewDescriptor {
                                    label: None,
                                    format: None,
                                    dimension: Some(TextureViewDimension::Cube),
                                    usage: None,
                                    aspect: TextureAspect::All,
                                    base_mip_level: 0,
                                    mip_level_count: None,
                                    base_array_layer: 0,
                                    array_layer_count: None,
                                });
                                let handle = images.add(img);
                                commands.insert_resource(PendingSkybox(handle));
                                info!("[MAP] skybox.webp decoded as cubemap — will apply to camera");
                            }
                            Err(e) => {
                                warn!("[MAP] skybox.webp is not a valid 6-face vertical strip ({:?}); skipping", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("[MAP] Failed to decode skybox.webp: {:?}; skipping", e);
                    }
                }
            }

            // Insert all extracted files into the in-memory asset source.
            for (path_str, data) in &extracted.files {
                map_dir.0.insert_asset(Path::new(path_str), data.clone());
            }

            // Begin loading scene.glb — with GLTF→Bevy coordinate conversion enabled.
            // GLTF uses +Z-forward / −X-right; Bevy uses −Z-forward / +X-right.
            // rotate_scene_entity applies a 180° Y rotation to the scene root so
            // normals, lighting and geometry all align with Bevy's coordinate system.
            let scene_handle: Handle<Gltf> = asset_server.load_with_settings(
                "map://scene.glb",
                |s: &mut GltfLoaderSettings| {
                    s.convert_coordinates = Some(GltfConvertCoordinates {
                        rotate_scene_entity: true,
                        rotate_meshes: false,
                    });
                },
            );
            commands.insert_resource(LoadingMapHandles {
                scene: scene_handle,
                scene_loaded: false,
            });
        }
    }
}

/// Watches `AssetEvent<Gltf>` to detect when both scene files are ready, then
/// swaps out the hardcoded map for the downloaded one.
fn poll_gltf_loaded(
    loading: Option<ResMut<LoadingMapHandles>>,
    mut gltf_events: MessageReader<AssetEvent<Gltf>>,
    gltf_assets: Res<Assets<Gltf>>,
    mut commands: Commands,
    hardcoded_query: Query<Entity, With<HardcodedMap>>,
    dynamic_query: Query<Entity, With<DynamicMap>>,
    mut label_query: Query<&mut Text, With<MapLoadingLabel>>,
) {
    let Some(mut loading) = loading else {
        // Drain events even when we're not waiting.
        for _ in gltf_events.read() {}
        return;
    };

    for event in gltf_events.read() {
        if let AssetEvent::LoadedWithDependencies { id } = event {
            if *id == loading.scene.id() {
                loading.scene_loaded = true;
                info!("[MAP] scene.glb loaded");
            }
        }
    }

    if !loading.scene_loaded {
        return;
    }

    info!("[MAP] GLTF ready — swapping map");

    // Despawn the previous dynamic map (if any).
    for entity in dynamic_query.iter() {
        commands.entity(entity).despawn();
    }
    // Despawn the hardcoded placeholder map.
    for entity in hardcoded_query.iter() {
        commands.entity(entity).despawn();
    }

    // Spawn the visual scene.  ColliderConstructorHierarchy is NOT added here
    // because Bevy's SceneSpawner won't instantiate the child entities until
    // the next frame's PreUpdate — see `attach_map_colliders` below.
    if let Some(gltf) = gltf_assets.get(&loading.scene) {
        let scene_handle = gltf
            .default_scene
            .clone()
            .or_else(|| gltf.scenes.first().cloned())
            .expect("scene.glb has no scenes");

        let map_entity = commands.spawn((
            Name::new("DynamicMap"),
            DynamicMap,
            SceneRoot(scene_handle),
        )).id();
        commands.insert_resource(PendingMapCollider(map_entity));
        info!("[MAP] Map scene spawned; waiting for SceneSpawner before attaching colliders");
    }

    // Keep the overlay up — it will be removed by attach_map_colliders once
    // the colliders exist and the player has been teleported to the floor.
    for mut text in label_query.iter_mut() {
        **text = "Setting up physics...".to_string();
    }

    commands.remove_resource::<LoadingMapHandles>();
}

/// Waits until Bevy's SceneSpawner has instantiated the map scene's child
/// entities, then manually traverses every descendant, creates trimesh
/// colliders directly from each mesh, and teleports the player to a spawn
/// point before removing the loading overlay.
///
/// We avoid `ColliderConstructorHierarchy` here because avian3d marks the
/// hierarchy "done" in the same PostUpdate pass it is added — if the GLTF
/// scene root entity happens to have children at that point but the mesh
/// assets aren't surfaced correctly, no colliders are created and the bug
/// is silent.  Creating colliders explicitly lets us confirm success and
/// retry the next frame if mesh data isn't available yet.
fn attach_map_colliders(
    pending: Option<Res<PendingMapCollider>>,
    meshes: Res<Assets<Mesh>>,
    children_of: Query<&Children>,
    mesh_query: Query<&Mesh3d>,
    spawn_points: Res<SpawnPoints>,
    mut player_query: Query<(&mut Transform, &mut LinearVelocity), With<LocalPlayer>>,
    mut overlay_query: Query<(Entity, &mut Visibility), With<MapLoadingOverlay>>,
    mut label_query: Query<(Entity, &mut Visibility), With<MapLoadingLabel>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else { return };

    // Collect every descendant entity that carries a Mesh3d handle.
    let mut stack = vec![pending.0];
    let mut mesh_entities: Vec<(Entity, Handle<Mesh>)> = Vec::new();
    while let Some(entity) = stack.pop() {
        if let Ok(mesh3d) = mesh_query.get(entity) {
            mesh_entities.push((entity, mesh3d.0.clone()));
        }
        if let Ok(children) = children_of.get(entity) {
            stack.extend(children.iter());
        }
    }

    if mesh_entities.is_empty() {
        return; // Scene not instantiated yet — retry next frame.
    }

    // Build trimesh colliders from mesh data.  If a mesh asset isn't ready
    // yet (shouldn't happen after LoadedWithDependencies, but guard anyway),
    // wait another frame rather than leaving the floor with no collider.
    let mut created = 0usize;
    for (entity, handle) in &mesh_entities {
        let Some(mesh) = meshes.get(handle) else {
            return; // Asset not ready — retry next frame.
        };
        match Collider::trimesh_from_mesh(mesh) {
            Some(collider) => {
                commands.entity(*entity).insert((collider, RigidBody::Static));
                created += 1;
            }
            None => {
                warn!("[MAP] trimesh_from_mesh returned None for {:?} — skipping", entity);
            }
        }
    }

    info!("[MAP] Created {} trimesh collider(s) from {} mesh(es)", created, mesh_entities.len());
    commands.remove_resource::<PendingMapCollider>();

    // Teleport the player onto the now-solid floor.
    let spawn_pos = pick_spawn_point(&spawn_points);
    for (mut transform, mut velocity) in player_query.iter_mut() {
        transform.translation = spawn_pos;
        *velocity = LinearVelocity::default();
        info!("[MAP] Player teleported to spawn {:?}", spawn_pos);
    }

    // Floor is solid and player is positioned — safe to reveal the scene.
    // We hide entities immediately via Visibility::Hidden (synchronous component
    // mutation) and also queue a deferred despawn for cleanup.  The immediate hide
    // is crucial: if the deferred despawn fails (e.g. because another system already
    // queued a despawn for the same entity in this frame), the entity is at least
    // invisible from the very next render frame onward.
    let overlay_n = overlay_query.iter().count();
    let label_n = label_query.iter().count();
    info!("[MAP] Revealing scene ({} overlay, {} label entities dismissed)", overlay_n, label_n);
    hide_and_despawn_overlay(&mut overlay_query, &mut label_query, &mut commands);
}

/// Attaches the `Skybox` component to the player's camera once the cubemap
/// image has been prepared by `poll_download`.
fn apply_skybox(
    pending: Option<Res<PendingSkybox>>,
    camera_query: Query<Entity, With<PlayerCamera>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else { return };
    let Ok(camera_entity) = camera_query.single() else { return };

    commands.entity(camera_entity).insert(Skybox {
        image: pending.0.clone(),
        brightness: 1000.0,
        rotation: Quat::IDENTITY,
    });
    commands.remove_resource::<PendingSkybox>();
    info!("[MAP] Skybox attached to camera");
}

/// Immediately hides the loading overlay and its text label by setting
/// `Visibility::Hidden` directly (synchronous — takes effect this render
/// frame), then queues a deferred `despawn()` for cleanup.
///
/// Hiding synchronously is important because `despawn()` is deferred through
/// `Commands` — if another system in the same frame already queued a despawn
/// for the same entity, our despawn command will hit a stale entity ID and
/// log a warning.  In that race, the immediate `Visibility::Hidden` guarantees
/// the overlay disappears even if the despawn is a no-op.
fn hide_and_despawn_overlay(
    overlay_query: &mut Query<(Entity, &mut Visibility), With<MapLoadingOverlay>>,
    label_query: &mut Query<(Entity, &mut Visibility), With<MapLoadingLabel>>,
    commands: &mut Commands,
) {
    let overlay_entities: Vec<Entity> = overlay_query
        .iter_mut()
        .map(|(entity, mut vis)| {
            *vis = Visibility::Hidden;
            entity
        })
        .collect();
    let label_entities: Vec<Entity> = label_query
        .iter_mut()
        .map(|(entity, mut vis)| {
            *vis = Visibility::Hidden;
            entity
        })
        .collect();
    for entity in overlay_entities {
        commands.entity(entity).despawn();
    }
    for entity in label_entities {
        commands.entity(entity).despawn();
    }
}

fn pick_spawn_point(spawn_points: &SpawnPoints) -> Vec3 {
    let points = &spawn_points.0;
    if points.is_empty() {
        return Vec3::new(0.0, 2.0, 0.0);
    }
    let mut buf = [0u8; 8];
    getrandom::getrandom(&mut buf).unwrap_or(());
    let idx = u64::from_le_bytes(buf) as usize % points.len();
    points[idx]
}

// ─── Helper: parse "x y z" spawn-point line ──────────────────────────────────

fn parse_vec3_line(line: &str) -> Option<Vec3> {
    let mut parts = line.split_whitespace();
    let x: f32 = parts.next()?.parse().ok()?;
    let y: f32 = parts.next()?.parse().ok()?;
    let z: f32 = parts.next()?.parse().ok()?;
    Some(Vec3::new(x, y, z))
}

// ─── Shared extraction logic ──────────────────────────────────────────────────

fn extract_archive(compressed: &[u8]) -> Result<ExtractedMap, String> {
    use ruzstd::decoding::StreamingDecoder;
    use std::io::Read;

    // Decompress zstd stream.
    let mut decoder = StreamingDecoder::new(compressed).map_err(|e| e.to_string())?;
    let mut tar_bytes = Vec::new();
    decoder.read_to_end(&mut tar_bytes).map_err(|e| e.to_string())?;

    // Extract tar.
    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    let mut files = HashMap::new();

    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let raw_path = entry.path().map_err(|e| e.to_string())?.into_owned();
        // Normalise path separators and strip any leading "./" prefix.
        let path_str = raw_path
            .to_string_lossy()
            .trim_start_matches("./")
            .replace('\\', "/");

        // Skip directory entries — their paths end with '/' (or have no
        // file_name component), and Dir::insert_asset would panic on the
        // file_name().unwrap() it performs internally.
        if Path::new(&path_str).file_name().is_none() {
            continue;
        }

        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        files.insert(path_str, data);
    }

    Ok(ExtractedMap { files })
}

// ─── Native download ──────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn native_download_and_extract(url: &str) -> Result<ExtractedMap, String> {
    use std::io::{BufReader, Read};
    use ureq::tls::{RootCerts, TlsConfig};

    info!("[MAP] (native) downloading {}", url);

    let agent = ureq::Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent();

    let mut response = agent.get(url).call().map_err(|e| e.to_string())?;
    let mut compressed = Vec::new();
    BufReader::new(response.body_mut().with_config().reader())
        .read_to_end(&mut compressed)
        .map_err(|e| e.to_string())?;

    info!("[MAP] downloaded {} bytes; extracting", compressed.len());
    extract_archive(&compressed)
}

// ─── WASM download ────────────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
async fn wasm_download_and_extract(url: &str) -> Result<ExtractedMap, String> {
    use js_sys::{ArrayBuffer, Uint8Array};
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::Response;

    let global: js_sys::Object = js_sys::global().unchecked_into();

    // Detect window vs worker global scope.
    let fetch_promise = if js_sys::Reflect::get(&global, &"Window".into())
        .map(|v| !v.is_undefined())
        .unwrap_or(false)
    {
        let window: web_sys::Window = global.unchecked_into();
        window.fetch_with_str(url)
    } else {
        let worker: web_sys::WorkerGlobalScope = global.unchecked_into();
        worker.fetch_with_str(url)
    };

    let resp_value = JsFuture::from(fetch_promise)
        .await
        .map_err(|e| format!("fetch error: {:?}", e))?;
    let resp: Response = resp_value
        .dyn_into()
        .map_err(|_| "fetch did not return a Response".to_string())?;

    if resp.status() != 200 {
        return Err(format!("HTTP {} for {}", resp.status(), url));
    }

    let ab_promise = resp
        .array_buffer()
        .map_err(|e| format!("array_buffer() error: {:?}", e))?;
    let ab_value = JsFuture::from(ab_promise)
        .await
        .map_err(|e| format!("array_buffer await error: {:?}", e))?;
    let ab: ArrayBuffer = ab_value
        .dyn_into()
        .map_err(|_| "expected ArrayBuffer".to_string())?;

    let compressed = Uint8Array::new(&ab).to_vec();
    extract_archive(&compressed)
}
