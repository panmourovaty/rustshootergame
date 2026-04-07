/// Client network plugin using lightyear 0.26's entity-based API.
///
/// The client connects by spawning an entity with:
///   - `UdpIo::default()` — the raw UDP IO layer
///   - `LocalAddr(bind_addr)` — local port (OS-assigned with port 0)
///   - `NetcodeClient::new(auth, config)` — secure netcode connection

use bevy::prelude::*;
// lightyear::prelude::client::* re-exports ClientPlugins, NetcodeClient, NetcodeConfig (client).
use lightyear::prelude::client::*;
// lightyear::prelude::* re-exports LocalAddr, UdpIo, Authentication, Connected, etc.
use lightyear::prelude::*;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use super::protocol::PROTOCOL_ID;

// ─── Plugin ─────────────────────────────────────────────────────────────────

pub struct ClientNetworkPlugin {
    pub server_addr: SocketAddr,
}

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        // Add the lightyear 0.25 client plugin group.
        app.add_plugins(ClientPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / 64.0),
        });

        // Store the server address for the startup system.
        app.insert_resource(TargetServer(self.server_addr));

        app.add_systems(Startup, spawn_client_entity);
        app.add_systems(Update, log_connection_status);

        info!("Client network plugin registered (server {})", self.server_addr);
    }
}

// ─── Resources ──────────────────────────────────────────────────────────────

#[derive(Resource)]
struct TargetServer(SocketAddr);

// ─── Systems ────────────────────────────────────────────────────────────────

/// Spawns the client entity that initiates the connection.
fn spawn_client_entity(mut commands: Commands, target: Res<TargetServer>) {
    // Bind to any available local port.
    let local_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);

    let auth = Authentication::Manual {
        server_addr: target.0,
        client_id: 1u64,
        private_key: [0u8; 32],
        protocol_id: PROTOCOL_ID,
    };

    match NetcodeClient::new(auth, NetcodeConfig::default()) {
        Ok(netcode_client) => {
            commands.spawn((
                Name::new("GameClient"),
                UdpIo::default(),
                LocalAddr(local_addr),
                netcode_client,
            ));
            info!("Client entity spawned, connecting to {}", target.0);
        }
        Err(e) => {
            error!("Failed to create NetcodeClient: {:?}", e);
        }
    }
}

/// Logs when our client entity becomes connected.
fn log_connection_status(
    query: Query<Entity, Added<Connected>>,
) {
    for entity in query.iter() {
        info!("Connected to server! Entity {:?}", entity);
    }
}
