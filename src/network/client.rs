/// Client network plugin using lightyear 0.26's entity-based API.
///
/// The server address and username are read from the `PlayerProfile` resource
/// that the connect screen populates before transitioning to `GameState::Playing`.

use bevy::prelude::*;
use lightyear::prelude::client::*;
use lightyear::prelude::*;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use super::protocol::PROTOCOL_ID;
use crate::game::{GameState, PlayerProfile};

// ─── Plugin ─────────────────────────────────────────────────────────────────

/// Add this to the client app.  No constructor arguments needed — connection
/// details come from the `PlayerProfile` resource filled by the connect screen.
pub struct ClientNetworkPlugin;

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ClientPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / 64.0),
        });

        // Spawn the connection entity only after the user has connected.
        app.add_systems(OnEnter(GameState::Playing), spawn_client_entity);
        app.add_systems(Update, log_connection_status);

        info!("ClientNetworkPlugin registered");
    }
}

// ─── Systems ────────────────────────────────────────────────────────────────

/// Spawns the lightyear client entity that initiates the UDP connection.
fn spawn_client_entity(mut commands: Commands, profile: Res<PlayerProfile>) {
    // Normalise: append default port if the stored address has none.
    let addr_str = if profile.server_addr.contains(':') {
        profile.server_addr.clone()
    } else {
        format!("{}:7777", profile.server_addr)
    };

    let server_addr: SocketAddr = match addr_str.parse() {
        Ok(a) => a,
        Err(e) => {
            error!("Invalid server address '{}': {}", addr_str, e);
            return;
        }
    };

    let local_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);

    let auth = Authentication::Manual {
        server_addr,
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
            info!(
                "Client entity spawned — connecting to {} as '{}'",
                server_addr, profile.username
            );
        }
        Err(e) => {
            error!("Failed to create NetcodeClient: {:?}", e);
        }
    }
}

fn log_connection_status(query: Query<Entity, Added<Connected>>) {
    for entity in query.iter() {
        info!("Connected to server! Entity {:?}", entity);
    }
}
