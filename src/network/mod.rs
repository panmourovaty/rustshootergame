/// Network subsystem — gated behind the `networking` feature flag.
///
/// Uses lightyear 0.26 (Bevy 0.18 compatible).

#[cfg(feature = "networking")]
pub mod protocol;

#[cfg(feature = "networking")]
pub mod server;

#[cfg(feature = "networking")]
pub mod client;

// Provide empty stubs when the feature is disabled so the module tree
// still compiles without any `#[cfg]` noise at call sites.
#[cfg(not(feature = "networking"))]
pub mod server {
    use bevy::prelude::*;

    pub struct ServerNetworkPlugin {
        pub port: u16,
    }

    impl Plugin for ServerNetworkPlugin {
        fn build(&self, _app: &mut App) {}
    }
}

#[cfg(not(feature = "networking"))]
pub mod client {
    use bevy::prelude::*;

    /// No-op stub used when the `networking` feature is disabled.
    pub struct ClientNetworkPlugin;

    impl Plugin for ClientNetworkPlugin {
        fn build(&self, _app: &mut App) {}
    }
}
