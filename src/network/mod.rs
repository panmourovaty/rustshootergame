/// Network subsystem.
///
/// Uses lightyear 0.28 (Bevy 0.19 compatible).
///
/// Feature gates:
///   `networking`  - native builds: UDP + WebTransport client + server
///   `web`         - WASM builds: WebTransport client only (no UDP, no server)

// Protocol is shared by both native and WASM builds.
#[cfg(any(feature = "networking", feature = "web"))]
pub mod protocol;

// Server only exists for native builds.
#[cfg(feature = "networking")]
pub mod server;

// Client exists for native (non-server) and WASM builds.
// Excluded when building with `--features server` so that the server binary
// doesn't need to compile client-side game modules (pvp, weapon, etc.).
#[cfg(any(all(feature = "networking", not(feature = "server")), feature = "web"))]
pub mod client;

// ── Stubs when all networking is disabled ────────────────────────────────────

#[cfg(not(feature = "networking"))]
pub mod server {
    use bevy::prelude::*;

    pub struct ServerNetworkPlugin {
        pub port: u16,
        pub web_port: u16,
    }

    impl Plugin for ServerNetworkPlugin {
        fn build(&self, _app: &mut App) {}
    }
}

#[cfg(not(any(feature = "networking", feature = "web")))]
pub mod client {
    use bevy::prelude::*;

    pub struct ClientNetworkPlugin;

    impl Plugin for ClientNetworkPlugin {
        fn build(&self, _app: &mut App) {}
    }
}
