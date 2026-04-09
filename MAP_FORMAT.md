# Map Format — RustShooterGame

This document describes how to create a map package for RustShooterGame and how to serve it to players via the dedicated server.

---

## Overview

Maps are distributed as a **`.tar.zst`** file (a `tar` archive compressed with Zstandard).  
The server operator passes the HTTPS URL of this file to the server binary via `--map-url`.  
When a client connects, the server immediately sends the URL via the network protocol.  
The client downloads the archive, extracts it into memory, and loads the 3D assets through Bevy's asset pipeline.

---

## Quick Start

```bash
# 1. Build your map files (see below).

# 2. Create the archive.
tar -cf mymap.tar scene.glb collision.glb textures/ spawn_points.txt
zstd -19 mymap.tar -o mymap.tar.zst

# 3. Host the file on any HTTPS server (e.g. S3, Cloudflare R2, nginx, etc.)
#    and note the public URL.

# 4. Run the dedicated server with the map URL.
./server --map-url https://example.com/maps/mymap.tar.zst
```

---

## Archive Layout

```
mymap.tar.zst
├── scene.glb           ← REQUIRED — visual geometry, materials, lights
├── collision.glb       ← REQUIRED — invisible collision meshes for physics
├── textures/           ← OPTIONAL — external textures (if not embedded in GLB)
│   ├── floor_albedo.png
│   ├── wall_normal.png
│   └── …
└── spawn_points.txt    ← OPTIONAL — player spawn locations
```

All paths inside the archive are resolved relative to the archive root.  
Forward slashes are used regardless of the platform that created the archive.

---

## `scene.glb` — Visual Scene

This file is loaded as the visual representation of the map.  
The client spawns it as a Bevy scene, so **everything in it is rendered**.

### What to include

| Element | Notes |
|---|---|
| Meshes & materials | Standard PBR (Base Color, Metallic, Roughness, Normal map) |
| Embedded textures | Preferred — embed textures in the GLB to avoid separate files |
| Lights | Optional — the scene can contain point, spot, or directional lights |
| Sky / environment | Optional — can be included as a background mesh or sky dome |

### Blender export settings

1. **File → Export → glTF 2.0 (.glb/.gltf)**
2. Choose **`.glb`** (binary, single file — simplest).
3. Under **Include**: check **Selected Objects** only if you want to be selective.
4. Under **Data → Mesh**: enable **Compression** only if your runtime supports it   
   (Draco compression is not enabled by default in Bevy).
5. Under **Data → Images**: set **Format** to **JPEG** or **PNG** and keep **Export** as **Automatic**.  
   If you prefer external textures, set **Image → Export** to **None** and place the textures in the `textures/` directory of the archive.

### Coordinate system

Bevy uses a **right-handed Y-up** coordinate system.  
glTF 2.0 also uses right-handed Y-up, so no conversion is needed.  
In Blender the default export converts from Blender's Z-up automatically.

---

## `collision.glb` — Physics Collision Scene

This file is loaded **invisible** and every mesh in it receives a **trimesh collider** via Avian3D's `ColliderConstructorHierarchy`.

### Guidelines

- **Keep it low-poly.** Trimesh colliders are exact but expensive — use simplified geometry.
- **No materials needed.** The scene is never rendered; materials are ignored.
- **One object = one collider.** Each mesh object in the GLTF becomes a separate `RigidBody::Static` trimesh collider.
- **Convex shapes are preferable** where possible (boxes, capsules, ramps) — trimesh collision detection is more expensive than convex-hull collision.

### Blender workflow

1. Duplicate your visual meshes into a new collection called `Collision`.
2. For each mesh, use **Decimate** or **Remesh** to reduce the polygon count dramatically.
3. Flat-shaded, box-approximated geometry is perfectly fine.
4. Export **only this collection** to `collision.glb`.

### Naming convention (optional)

While all meshes automatically receive colliders, you can name objects descriptively for debugging:

| Prefix | Meaning |
|---|---|
| `col_floor` | Floor plane |
| `col_wall_*` | Wall segments |
| `col_ramp_*` | Ramps / slopes |

---

## `textures/` — External Textures (optional)

If you chose **not** to embed textures inside the GLB, place them here and reference them with relative paths in your GLTF JSON.

```
textures/
├── floor_albedo.png        ← Base color / albedo
├── floor_normal.png        ← Normal map (OpenGL convention, Y-up)
├── floor_orm.png           ← Occlusion (R) / Roughness (G) / Metallic (B)
└── wall_albedo.png
```

The GLTF file should reference them as `../textures/floor_albedo.png` (relative to `scene.gltf`, if you use the non-binary `.gltf` format) or simply `textures/floor_albedo.png` relative to the archive root.

> **Tip:** Embedding textures in the `.glb` avoids any path resolution issues and is recommended for most maps.

---

## `spawn_points.txt` — Player Spawn Locations (optional)

A plain-text file with one spawn point per line.  
Each line must contain three space-separated floating-point numbers: **X Y Z**.

```
-15.0 2.0 -15.0
 15.0 2.0  15.0
-15.0 2.0  15.0
 15.0 2.0 -15.0
```

- **X** and **Z** are horizontal axes.
- **Y** is the vertical axis (height).
- A Y value of about `2.0` places the spawn point one player-height above the floor.
- Leading/trailing whitespace and empty lines are ignored.

If the file is absent, the built-in spawn points are used (the four corners of the default map).

---

## Server Usage

```
./server [OPTIONS]

Options:
  --port <PORT>         UDP port for native clients         [default: 7777]
  --web-port <PORT>     WebTransport port for WASM clients  [default: 7778]
  --map-url <URL>       HTTPS URL of the map .tar.zst file  [optional]
```

### Example

```bash
./server --port 7777 --web-port 7778 \
         --map-url https://cdn.example.com/maps/dust2.tar.zst
```

When `--map-url` is omitted the server does **not** send a `MapUrlMsg`, and all clients continue to use the built-in placeholder map.

---

## Changing the Map at Runtime

Currently, the map URL is set once at server start.  Sending a new `MapUrlMsg` to all connected clients (e.g. at round end) is possible through the network protocol — see `src/network/protocol.rs` — but no automatic round-cycling logic is implemented yet.

---

## Hosting the Archive

Any standard HTTPS file server works.  Make sure to:

1. Serve with **Content-Type: application/octet-stream** (or any binary type).
2. Set **CORS headers** if clients are connecting from a browser (WASM build):
   ```
   Access-Control-Allow-Origin: *
   ```
3. Keep the file **reasonably small** (< 50 MB recommended).  
   Zstandard compression is very efficient for 3D assets — a typical small map  
   compresses to well under 10 MB.

---

## Authoring Checklist

- [ ] `scene.glb` exported from Blender with Y-up, right-handed axes
- [ ] `collision.glb` contains low-poly, closed meshes (no gaps)
- [ ] All textures either embedded in GLB or present under `textures/`
- [ ] `spawn_points.txt` has at least 2 spawn locations, Y ≈ player height above floor
- [ ] Archive created with `tar` + `zstd` (NOT gzip or bzip2)
- [ ] File hosted over **HTTPS** and publicly accessible
- [ ] CORS header set if browser (WASM) clients will connect
- [ ] Tested by running `./server --map-url <your-url>` locally and connecting a client
