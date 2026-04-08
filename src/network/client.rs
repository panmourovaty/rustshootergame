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
///
/// Playing state:
///   OnEnter(Playing)    → send JoinMsg to server.
///   Update(Playing)     → throttled position updates, read server messages.

use bevy::prelude::*;
use lightyear::prelude::client::*;
use lightyear::prelude::*;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};

#[cfg(target_arch = "wasm32")]
use std::net::SocketAddr;

use super::protocol::{
    GameChannel, HitMsg, JoinMsg, KillNotifyMsg, PlayerJoinMsg, PlayerLeaveMsg,
    PosChannel, PosUpdateMsg, ProtocolPlugin, RelayedPosMsg, TakeDamageMsg, PROTOCOL_ID,
};
use crate::game::{ConnectionError, GameState, PlayerProfile};
use crate::player::LocalPlayer;
use crate::pvp::{
    LocalPlayerDamaged, RemoteKillEvent, RemotePlayerJoined, RemotePlayerLeft,
    RemotePlayerMoved,
};
use crate::weapon::RemoteHitEvent;

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

        // Register messages and channels AFTER ClientPlugins.
        app.add_plugins(ProtocolPlugin);

        app.add_systems(OnEnter(GameState::Connecting), start_connecting);
        app.add_systems(OnExit(GameState::Connecting), cleanup_pending_client);
        app.add_systems(
            Update,
            (check_connected, tick_timeout).run_if(in_state(GameState::Connecting)),
        );

        // Playing-state systems.
        app.add_systems(OnEnter(GameState::Playing), send_join_msg);
        app.add_systems(
            Update,
            (
                send_position_updates,
                process_server_messages,
                send_remote_hits,
            )
                .run_if(in_state(GameState::Playing)),
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

/// Tracks when we last sent a position update (for 50 ms throttle).
#[derive(Resource, Default)]
struct PosUpdateTimer(f32);

// ─── Connection systems ──────────────────────────────────────────────────────

fn start_connecting(mut commands: Commands, profile: Res<PlayerProfile>) {
    let addr_str = if profile.server_addr.contains(':') {
        profile.server_addr.clone()
    } else {
        format!("{}:7777", profile.server_addr)
    };

    let server_addr: SocketAddr = match resolve_addr(&addr_str) {
        Ok(a) => a,
        Err(e) => {
            error!("Cannot resolve '{}': {}", addr_str, e);
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

    commands.insert_resource(ConnectTimeout(Timer::from_seconds(15.0, TimerMode::Once)));
    commands.init_resource::<PosUpdateTimer>();
}

fn check_connected(
    query: Query<Entity, (With<PendingClient>, Added<Connected>)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if query.iter().next().is_some() {
        info!("Connected! Entering Playing state.");
        next_state.set(GameState::Playing);
    }
}

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

fn cleanup_pending_client(
    mut commands: Commands,
    query: Query<(Entity, Option<&Connected>), With<PendingClient>>,
) {
    for (entity, connected) in query.iter() {
        if connected.is_none() {
            commands.entity(entity).despawn();
        }
    }
    commands.remove_resource::<ConnectTimeout>();
}

// ─── Playing-state systems ───────────────────────────────────────────────────

/// Sends JoinMsg to the server as soon as we enter Playing.
fn send_join_msg(
    profile: Res<PlayerProfile>,
    mut client_query: Query<&mut MessageSender<JoinMsg>, (With<Client>, With<Connected>)>,
) {
    let msg = JoinMsg {
        client_id: profile.client_id,
        username: profile.username.clone(),
    };

    for mut sender in client_query.iter_mut() {
        sender.send::<GameChannel>(msg.clone());
        info!("Sent JoinMsg: id={} username='{}'", msg.client_id, msg.username);
    }
}

/// Sends PosUpdateMsg to the server at ~50 ms intervals (20 Hz).
fn send_position_updates(
    time: Res<Time>,
    mut timer: ResMut<PosUpdateTimer>,
    profile: Res<PlayerProfile>,
    player_query: Query<(&Transform, &crate::player::FpsController), With<LocalPlayer>>,
    mut client_query: Query<&mut MessageSender<PosUpdateMsg>, (With<Client>, With<Connected>)>,
) {
    timer.0 += time.delta_secs();
    if timer.0 < 0.05 {
        return;
    }
    timer.0 = 0.0;

    let Ok((transform, controller)) = player_query.single() else {
        return;
    };
    let pos = transform.translation;
    let yaw = controller.yaw;

    let msg = PosUpdateMsg {
        pos: [pos.x, pos.y, pos.z],
        yaw,
    };

    for mut sender in client_query.iter_mut() {
        sender.send::<PosChannel>(msg.clone());
    }
    let _ = profile; // profile.client_id unused here; server knows who sent it
}

/// Reads all server messages and fires the appropriate Bevy events.
fn process_server_messages(
    profile: Res<PlayerProfile>,
    mut client_query: Query<
        (
            &mut MessageReceiver<PlayerJoinMsg>,
            &mut MessageReceiver<PlayerLeaveMsg>,
            &mut MessageReceiver<RelayedPosMsg>,
            &mut MessageReceiver<TakeDamageMsg>,
            &mut MessageReceiver<KillNotifyMsg>,
        ),
        (With<Client>, With<Connected>),
    >,
    mut joined_events: MessageWriter<RemotePlayerJoined>,
    mut left_events: MessageWriter<RemotePlayerLeft>,
    mut moved_events: MessageWriter<RemotePlayerMoved>,
    mut damaged_events: MessageWriter<LocalPlayerDamaged>,
    mut kill_events: MessageWriter<RemoteKillEvent>,
) {
    for (
        mut join_rx,
        mut leave_rx,
        mut pos_rx,
        mut damage_rx,
        mut kill_rx,
    ) in client_query.iter_mut()
    {
        // PlayerJoinMsg
        for msg in join_rx.receive() {
            info!(
                "Server: player '{}' (id={}) joined",
                msg.username, msg.client_id
            );
            joined_events.write(RemotePlayerJoined {
                client_id: msg.client_id,
                username: msg.username.clone(),
                color: msg.color,
            });
        }

        // PlayerLeaveMsg
        for msg in leave_rx.receive() {
            info!("Server: player id={} left", msg.client_id);
            left_events.write(RemotePlayerLeft {
                client_id: msg.client_id,
            });
        }

        // RelayedPosMsg — skip our own updates to avoid overwriting local transform.
        for msg in pos_rx.receive() {
            if msg.client_id == profile.client_id {
                continue;
            }
            moved_events.write(RemotePlayerMoved {
                client_id: msg.client_id,
                pos: Vec3::from(msg.pos),
                yaw: msg.yaw,
            });
        }

        // TakeDamageMsg
        for msg in damage_rx.receive() {
            info!("Server: our HP is now {:.0}", msg.new_hp);
            damaged_events.write(LocalPlayerDamaged { new_hp: msg.new_hp });
        }

        // KillNotifyMsg
        for msg in kill_rx.receive() {
            info!(
                "Server: kill confirmed — {} killed {}",
                msg.killer_id, msg.victim_id
            );
            kill_events.write(RemoteKillEvent {
                killer_id: msg.killer_id,
                victim_id: msg.victim_id,
            });
        }
    }
}

/// Reads `RemoteHitEvent` (fired by weapon.rs when local player hits a remote
/// player) and sends `HitMsg` to the server for authoritative damage processing.
fn send_remote_hits(
    profile: Res<PlayerProfile>,
    mut hit_events: MessageReader<RemoteHitEvent>,
    mut client_query: Query<&mut MessageSender<HitMsg>, (With<Client>, With<Connected>)>,
) {
    for ev in hit_events.read() {
        let msg = HitMsg {
            killer_id: profile.client_id,
            victim_id: ev.victim_id,
            damage: ev.damage,
        };

        for mut sender in client_query.iter_mut() {
            sender.send::<GameChannel>(msg.clone());
        }

        info!(
            "Sent HitMsg: killer={} victim={} damage={:.0}",
            msg.killer_id, msg.victim_id, msg.damage
        );
    }
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
