/// Dedicated-server network plugin — lightyear 0.26 entity-based API.
///
/// Listens on two transports simultaneously:
///   UDP           — native clients (port `--port`, default 7777)
///   WebTransport  — browser/WASM clients (port `--web-port`, default 7778)
///
/// A self-signed TLS certificate is generated at startup for WebTransport.
/// The SHA-256 fingerprint is printed so operators can bake it into the WASM
/// build via the `RSG_CERT_DIGEST` compile-time environment variable.

use bevy::prelude::*;
use lightyear::prelude::server::*;
use lightyear::prelude::{Connected, LocalAddr};
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use aeronet_webtransport::wtransport::Identity;
use lightyear::prelude::server::WebTransportServerIo;

use super::protocol::PROTOCOL_ID;

// ─── Plugin ─────────────────────────────────────────────────────────────────

pub struct ServerNetworkPlugin {
    pub port: u16,
    pub web_port: u16,
}

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ServerPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / 64.0),
        });

        app.insert_resource(ServerPorts {
            udp: self.port,
            web: self.web_port,
        });
        app.add_systems(Startup, spawn_server_entities);
        app.add_systems(Update, log_client_connections);

        info!(
            "ServerNetworkPlugin registered (UDP :{}, WebTransport :{})",
            self.port, self.web_port
        );
    }
}

// ─── Resources ──────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct ServerPorts {
    pub udp: u16,
    pub web: u16,
}

// ─── Systems ────────────────────────────────────────────────────────────────

fn spawn_server_entities(mut commands: Commands, ports: Res<ServerPorts>) {
    let netcode_config = NetcodeConfig {
        protocol_id: PROTOCOL_ID,
        private_key: [0u8; 32],
        ..default()
    };

    // ── UDP listener (native clients) ────────────────────────────────────────
    let udp_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), ports.udp);
    commands.spawn((
        Name::new("GameServerUdp"),
        ServerUdpIo::default(),
        LocalAddr(udp_addr),
        NetcodeServer::new(netcode_config.clone()),
    ));
    info!("UDP listener on {}", udp_addr);

    // ── WebTransport listener (browser clients) ───────────────────────────────
    let wt_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), ports.web);

    let identity = Identity::self_signed(["localhost", "127.0.0.1", "::1"])
        .expect("failed to generate self-signed TLS certificate");

    // Print the SHA-256 fingerprint so the operator can set RSG_CERT_DIGEST.
    // Display format is dotted hex (aa:bb:cc:…); strip colons for the env var.
    let dotted = format!("{}", identity.certificate_chain().as_slice()[0].hash());
    let cert_digest = dotted.replace(':', "");
    info!(
        "WebTransport listener on {} | cert digest: {}",
        wt_addr, cert_digest
    );
    info!(
        "→ To build the WASM client: RSG_CERT_DIGEST={} cargo build \
         --target wasm32-unknown-unknown --no-default-features --features web \
         --profile wasm-release --bin client",
        cert_digest
    );

    commands.spawn((
        Name::new("GameServerWebTransport"),
        WebTransportServerIo {
            certificate: identity,
        },
        LocalAddr(wt_addr),
        NetcodeServer::new(netcode_config),
    ));
}

fn log_client_connections(query: Query<Entity, Added<Connected>>) {
    for entity in query.iter() {
        info!("Client connected: entity {:?}", entity);
    }
}
