/// Network subsystem — gated behind the `networking` feature flag.
///
/// The lightyear 0.25 API may require minor adjustments if breaking
/// changes landed after the dependency was locked.

#[cfg(feature = "networking")]
pub mod protocol;

#[cfg(feature = "networking")]
pub mod server;

#[cfg(feature = "networking")]
pub mod client;

// Provide empty stubs when the feature is disabled so the module tree
// still compiles without any `#[cfg]` noise at call sites in main.rs.
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
    use std::net::SocketAddr;

    pub struct ClientNetworkPlugin {
        pub server_addr: SocketAddr,
    }

    impl Plugin for ClientNetworkPlugin {
        fn build(&self, _app: &mut App) {}
    }
}
