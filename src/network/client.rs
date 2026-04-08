/// Client network plugin — lightyear 0.26.
///
/// Native builds use UDP; WASM builds use WebTransport.
///
/// Connection flow:
///   OnEnter(Connecting) → spawn the lightyear client entity and start a
///                         15-second timeout timer.
///   Update(Connecting)  → when Added<Connected> fires, go to Playing.
///                         When the timer expires, go back to ConnectScreen
///                         with a ConnectionError message.
///   OnExit(Connecting)  → clean up any pending (not-yet-connected) entity.

use bevy::prelude::*;
use lightyear::prelude::client::*;
use lightyear::prelude::*;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};

#[cfg(target_arch = "wasm32")]
use std::net::SocketAddr;

use super::protocol::PROTOCOL_ID;
use crate::game::{ConnectionError, GameState, PlayerProfile};

fn random_client_id() -> u64 {
    let mut buf = [0u8; 8];
    getrandom::getrandom(&mut buf).expect("getrandom failed");
    u64::from_le_bytes(buf)
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

pub struct ClientNetworkPlugin;

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ClientPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / 64.0),
        });

        app.add_systems(OnEnter(GameState::Connecting), start_connecting);
        app.add_systems(OnExit(GameState::Connecting), cleanup_pending_client);
        app.add_systems(
            Update,
            (check_connected, tick_timeout).run_if(in_state(GameState::Connecting)),
        );

        info!("ClientNetworkPlugin registered");
    }
}

// ─── Components / Resources ──────────────────────────────────────────────────

/// Marks the lightyear client entity while a connection attempt is in progress.
/// Despawned if the attempt fails or is cancelled; kept alive on success.
#[derive(Component)]
struct PendingClient;

/// Countdown to give up on the connection attempt.
#[derive(Resource)]
struct ConnectTimeout(Timer);

// ─── Systems ────────────────────────────────────────────────────────────────

fn start_connecting(mut commands: Commands, profile: Res<PlayerProfile>) {
    // Resolve the address (handles both IPs and domain names on native).
    // On WASM, to_socket_addrs() works for IP literals; the WebTransport URL
    // is what actually drives DNS in the browser.
    let addr_str = if profile.server_addr.contains(':') {
        profile.server_addr.clone()
    } else {
        format!("{}:7777", profile.server_addr)
    };

    let server_addr: SocketAddr = match resolve_addr(&addr_str) {
        Ok(a) => a,
        Err(e) => {
            error!("Cannot resolve '{}': {}", addr_str, e);
            // The timeout system will fire shortly and surface the error.
            return;
        }
    };

    let auth = Authentication::Manual {
        server_addr,
        client_id: random_client_id(),
        private_key: [0u8; 32],
        protocol_id: PROTOCOL_ID,
    };

    match NetcodeClient::new(auth, NetcodeConfig::default()) {
        Ok(netcode_client) => {
            spawn_transport(&mut commands, server_addr, netcode_client, &profile);
            info!("Connecting to {} as '{}'…", server_addr, profile.username);
        }
        Err(e) => {
            error!("Failed to create NetcodeClient: {:?}", e);
        }
    }

    // 15-second connection timeout.
    commands.insert_resource(ConnectTimeout(Timer::from_seconds(15.0, TimerMode::Once)));
}

/// Transition to Playing the frame a Connected component appears.
fn check_connected(
    query: Query<Entity, (With<PendingClient>, Added<Connected>)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if query.iter().next().is_some() {
        info!("Connected! Entering Playing state.");
        next_state.set(GameState::Playing);
    }
}

/// Give up after the timeout and send the player back to the connect screen.
fn tick_timeout(
    time: Res<Time>,
    mut timer: ResMut<ConnectTimeout>,
    mut next_state: ResMut<NextState<GameState>>,
    mut conn_error: ResMut<ConnectionError>,
) {
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        conn_error.0 = Some("Connection timed out.".to_string());
        next_state.set(GameState::ConnectScreen);
    }
}

/// Despawn any pending (unconnected) client entity and remove the timeout.
fn cleanup_pending_client(
    mut commands: Commands,
    query: Query<(Entity, Option<&Connected>), With<PendingClient>>,
) {
    for (entity, connected) in query.iter() {
        if connected.is_none() {
            commands.entity(entity).despawn();
        }
        // If already Connected, keep the entity alive for the Playing state.
    }
    commands.remove_resource::<ConnectTimeout>();
}

// ─── Address resolution ──────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn resolve_addr(addr_str: &str) -> Result<SocketAddr, String> {
    addr_str
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| format!("No address resolved for '{}'", addr_str))
}

#[cfg(target_arch = "wasm32")]
fn resolve_addr(addr_str: &str) -> Result<SocketAddr, String> {
    addr_str.parse::<SocketAddr>().map_err(|e| e.to_string())
}

// ─── Transport selection ─────────────────────────────────────────────────────

/// Native: UDP transport.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_transport(
    commands: &mut Commands,
    server_addr: SocketAddr,
    netcode_client: NetcodeClient,
    _profile: &PlayerProfile,
) {
    use lightyear::prelude::client::Connect;
    let local_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
    let entity = commands.spawn((
        Name::new("GameClient"),
        PendingClient,
        UdpIo::default(),
        LocalAddr(local_addr),
        netcode_client,
    )).id();
    // lightyear only initiates the handshake once it receives the Connect trigger.
    commands.trigger(Connect { entity });
    info!("Using UDP transport → {}", server_addr);
}

/// WASM: WebTransport transport.
///
/// The server address is formatted as an HTTPS URL for the browser's
/// WebTransport API.  `RSG_CERT_DIGEST` must be set at compile time to the
/// SHA-256 fingerprint (hex, no colons) of the server's self-signed cert.
/// Leave it empty when the server uses a CA-signed certificate.
#[cfg(target_arch = "wasm32")]
fn spawn_transport(
    commands: &mut Commands,
    server_addr: SocketAddr,
    netcode_client: NetcodeClient,
    _profile: &PlayerProfile,
) {
    use lightyear::prelude::client::{Connect, WebTransportClientIo};

    // Cert digest baked in at compile time via RSG_CERT_DIGEST env var.
    // Empty string = CA-signed cert (no pinning required).
    let certificate_digest = option_env!("RSG_CERT_DIGEST")
        .unwrap_or("")
        .to_string();

    let entity = commands.spawn((
        Name::new("GameClient"),
        PendingClient,
        WebTransportClientIo { certificate_digest },
        PeerAddr(server_addr),
        netcode_client,
    )).id();
    commands.trigger(Connect { entity });
    info!("Using WebTransport → https://{}", server_addr);
}
