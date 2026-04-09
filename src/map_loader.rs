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
use bevy::gltf::Gltf;
use bevy::prelude::*;
use avian3d::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::map::{HardcodedMap, SpawnPoints};

// ─── Public surface ──────────────────────────────────────────────────────────

/// The in-memory directory that backs the `map://` asset source.
/// Clone the inner `Dir` to insert files; it is `Arc`-backed so all clones
/// share the same storage.
#[derive(Resource, Clone)]
pub struct MapDir(pub Dir);

/// Fire this event (e.g. from the network client) to trigger a map download.
#[derive(Event, Clone)]
pub struct LoadMapFromUrl(pub String);

pub struct MapLoaderPlugin {
    /// A clone of the `Dir` that was already registered as the `map://`
    /// asset source in `main.rs` before `DefaultPlugins` was added.
    pub dir: Dir,
}

impl Plugin for MapLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MapDir(self.dir.clone()));
        app.add_event::<LoadMapFromUrl>();
        app.add_systems(
            Update,
            (handle_load_map_event, poll_download, poll_gltf_loaded),
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

/// Resource present while we wait for the GLTF handles to finish loading.
#[derive(Resource)]
struct LoadingMapHandles {
    scene: Handle<Gltf>,
    collision: Option<Handle<Gltf>>,
    scene_loaded: bool,
    collision_loaded: bool,
}

/// Marker for entities spawned by the dynamic map so they can be cleaned up
/// when a new map is loaded.
#[derive(Component)]
pub struct DynamicMap;

// ─── Systems ─────────────────────────────────────────────────────────────────

/// Reacts to `LoadMapFromUrl`, kicks off a background download, and installs
/// `PendingDownload` so `poll_download` can check on it every frame.
fn handle_load_map_event(
    mut events: EventReader<LoadMapFromUrl>,
    mut commands: Commands,
) {
    for event in events.read() {
        let url = event.0.clone();
        info!("[MAP] Starting download: {}", url);

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
    mut spawn_points: ResMut<SpawnPoints>,
    mut commands: Commands,
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
        }
        Ok(extracted) => {
            info!("[MAP] Extracted {} files; inserting into map:// source", extracted.files.len());

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

            // Insert all extracted files into the in-memory asset source.
            for (path_str, data) in &extracted.files {
                map_dir.0.insert_asset(Path::new(path_str), data.clone());
            }

            // Begin loading the GLTF assets.
            let scene_handle: Handle<Gltf> = asset_server.load("map://scene.glb");
            let collision_handle = if extracted.files.contains_key("collision.glb") {
                Some(asset_server.load::<Gltf>("map://collision.glb"))
            } else {
                warn!("[MAP] collision.glb not found in archive; no physics will be set up");
                None
            };

            commands.insert_resource(LoadingMapHandles {
                scene: scene_handle,
                collision: collision_handle,
                scene_loaded: false,
                collision_loaded: false,
            });
        }
    }
}

/// Watches `AssetEvent<Gltf>` to detect when both scene files are ready, then
/// swaps out the hardcoded map for the downloaded one.
fn poll_gltf_loaded(
    loading: Option<ResMut<LoadingMapHandles>>,
    mut gltf_events: EventReader<AssetEvent<Gltf>>,
    gltf_assets: Res<Assets<Gltf>>,
    mut commands: Commands,
    hardcoded_query: Query<Entity, With<HardcodedMap>>,
    dynamic_query: Query<Entity, With<DynamicMap>>,
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
            if let Some(col) = &loading.collision {
                if *id == col.id() {
                    loading.collision_loaded = true;
                    info!("[MAP] collision.glb loaded");
                }
            }
        }
    }

    let collision_ready = loading.collision.is_none() || loading.collision_loaded;
    if !loading.scene_loaded || !collision_ready {
        return;
    }

    info!("[MAP] All GLTF files ready — swapping map");

    // Despawn the previous dynamic map (if any).
    for entity in dynamic_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    // Despawn the hardcoded placeholder map.
    for entity in hardcoded_query.iter() {
        commands.entity(entity).despawn_recursive();
    }

    // Spawn visual scene.
    if let Some(gltf) = gltf_assets.get(&loading.scene) {
        let scene_handle = gltf
            .default_scene
            .clone()
            .or_else(|| gltf.scenes.first().cloned())
            .expect("scene.glb has no scenes");

        commands.spawn((
            Name::new("DynamicMap_Visual"),
            DynamicMap,
            SceneRoot(scene_handle),
        ));
        info!("[MAP] Visual scene spawned");
    }

    // Spawn collision scene (invisible, physics only).
    if let Some(col_handle) = &loading.collision {
        if let Some(gltf) = gltf_assets.get(col_handle) {
            let scene_handle = gltf
                .default_scene
                .clone()
                .or_else(|| gltf.scenes.first().cloned())
                .expect("collision.glb has no scenes");

            commands.spawn((
                Name::new("DynamicMap_Collision"),
                DynamicMap,
                SceneRoot(scene_handle),
                // Auto-generate trimesh colliders for every mesh in the scene.
                ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh),
                // Render nothing — this scene is physics-only.
                Visibility::Hidden,
            ));
            info!("[MAP] Collision scene spawned with ColliderConstructorHierarchy");
        }
    }

    commands.remove_resource::<LoadingMapHandles>();
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
    use ruzstd::StreamingDecoder;
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
