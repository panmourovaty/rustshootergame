/// Dedicated-server network plugin — lightyear 0.26 entity-based API.
///
/// A server is created by spawning an entity with:
///   - `ServerUdpIo::default()` — raw UDP transport
///   - `LocalAddr(addr)` — bind address
///   - `NetcodeServer::new(cfg)` — secure netcode handshake
/// The `Server` marker component is auto-inserted via `#[require(Server)]`.

use bevy::prelude::*;
// lightyear::prelude::server::* re-exports ServerPlugins, ServerUdpIo,
// NetcodeServer, NetcodeConfig (server variant), and connection types.
use lightyear::prelude::server::*;
// lightyear::prelude::* re-exports LocalAddr (from aeronet_io), Connected, etc.
use lightyear::prelude::{Connected, LocalAddr};
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use super::protocol::PROTOCOL_ID;

// ─── Plugin ─────────────────────────────────────────────────────────────────

pub struct ServerNetworkPlugin {
    pub port: u16,
}

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ServerPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / 64.0),
        });

        app.insert_resource(ServerPort(self.port));
        app.add_systems(Startup, spawn_server_entity);
        app.add_systems(Update, log_client_connections);

        info!("ServerNetworkPlugin registered (port {})", self.port);
    }
}

// ─── Resources ──────────────────────────────────────────────────────────────

#[derive(Resource)]
struct ServerPort(u16);

// ─── Systems ────────────────────────────────────────────────────────────────

fn spawn_server_entity(mut commands: Commands, port: Res<ServerPort>) {
    let bind_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port.0);

    let netcode_config = NetcodeConfig {
        protocol_id: PROTOCOL_ID,
        private_key: [0u8; 32],
        ..default()
    };

    commands.spawn((
        Name::new("GameServer"),
        ServerUdpIo::default(),
        LocalAddr(bind_addr),
        NetcodeServer::new(netcode_config),
    ));
    info!("Server entity spawned, listening on {}", bind_addr);
}

/// Logs whenever a client entity's connection becomes established.
fn log_client_connections(query: Query<Entity, Added<Connected>>) {
    for entity in query.iter() {
        info!("Client connected: entity {:?}", entity);
    }
}
