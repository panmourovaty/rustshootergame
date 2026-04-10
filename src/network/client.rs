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
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};

#[cfg(target_arch = "wasm32")]
use std::net::SocketAddr;

// WASM-only imports for the hostname-based WebTransport path.
#[cfg(target_arch = "wasm32")]
use lightyear_aeronet::AeronetLinkOf;
#[cfg(target_arch = "wasm32")]
use aeronet_webtransport::client::{ClientConfig as WtClientConfig, WebTransportClient};

use super::protocol::{
    GameChannel, HitMsg, JoinMsg, KillNotifyMsg, MapUrlMsg, PlayerJoinMsg, PlayerLeaveMsg,
    PosChannel, PosUpdateMsg, ProtocolPlugin, RelayedPosMsg, TakeDamageMsg, PROTOCOL_ID,
};
use crate::map_loader::LoadMapFromUrl;
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

        app.add_systems(OnEnter(GameState::ConnectScreen), disconnect_client);
        app.add_systems(OnEnter(GameState::Connecting), start_connecting);
        app.add_systems(OnExit(GameState::Connecting), cleanup_pending_client);
        app.add_systems(
            Update,
            (check_connected, tick_timeout).run_if(in_state(GameState::Connecting)),
        );

        // WASM: custom observer that connects via a string URL instead of
        // PeerAddr(SocketAddr), supporting both IP addresses and hostnames.
        #[cfg(target_arch = "wasm32")]
        app.add_observer(wt_hostname_link);

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

/// WASM-only: carry the full WebTransport URL string and the certificate digest
/// so our custom `wt_hostname_link` observer can connect using a hostname
/// (lightyear's built-in `WebTransportClientIo` only accepts `PeerAddr(SocketAddr)`
/// which cannot represent hostnames).
#[cfg(target_arch = "wasm32")]
#[derive(Component)]
#[require(Link)]
struct WebTransportHostname {
    url: String,
    cert_digest: String,
}

// ─── Connection systems ──────────────────────────────────────────────────────

fn start_connecting(
    mut commands: Commands,
    profile: Res<PlayerProfile>,
    mut next_state: ResMut<NextState<GameState>>,
    mut conn_error: ResMut<ConnectionError>,
) {
    // Insert ConnectTimeout FIRST — before any early return — so that
    // tick_timeout never panics with "Resource does not exist".
    commands.insert_resource(ConnectTimeout(Timer::from_seconds(15.0, TimerMode::Once)));
    commands.init_resource::<PosUpdateTimer>();

    let addr_str = append_default_port(&profile.server_addr);

    // ── Platform-specific connection setup ────────────────────────────────────

    #[cfg(not(target_arch = "wasm32"))]
    {
        let server_addr: SocketAddr = match resolve_addr(&addr_str) {
            Ok(a) => a,
            Err(e) => {
                let msg = format!("Cannot resolve '{}': {}", addr_str, e);
                error!("{}", msg);
                conn_error.0 = Some(msg);
                next_state.set(GameState::ConnectScreen);
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
                let msg = format!("Failed to create NetcodeClient: {:?}", e);
                error!("{}", msg);
                conn_error.0 = Some(msg);
                next_state.set(GameState::ConnectScreen);
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        // In WASM, SocketAddr::parse() only works for IP:port.  For hostnames
        // (e.g. "rsgsrv.hejl.xyz:7778") we cannot do synchronous DNS.
        // We use a dummy SocketAddr carrying the right port for the netcode
        // connect-token; the server does not validate server_addresses in the
        // token (that check is commented out in lightyear_netcode), so any
        // valid SocketAddr is accepted.
        let netcode_addr = wasm_netcode_addr(&addr_str);

        let auth = Authentication::Manual {
            server_addr: netcode_addr,
            client_id: random_client_id(),
            private_key: [0u8; 32],
            protocol_id: PROTOCOL_ID,
        };

        match NetcodeClient::new(auth, NetcodeConfig::default()) {
            Ok(netcode_client) => {
                spawn_transport(&mut commands, &addr_str, netcode_client, &profile);
                info!("Connecting to https://{} as '{}'…", addr_str, profile.username);
            }
            Err(e) => {
                let msg = format!("Failed to create NetcodeClient: {:?}", e);
                error!("{}", msg);
                conn_error.0 = Some(msg);
                next_state.set(GameState::ConnectScreen);
            }
        }
    }
}

fn check_connected(
    query: Query<Entity, (With<PendingClient>, With<Connected>)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if query.iter().next().is_some() {
        info!("Connected! Entering Playing state.");
        next_state.set(GameState::Playing);
    }
}

fn tick_timeout(
    time: Res<Time>,
    // Wrapped in Option: ConnectTimeout is inserted by start_connecting but
    // there is a window (same frame as OnEnter) where it may not yet exist.
    timer: Option<ResMut<ConnectTimeout>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut conn_error: ResMut<ConnectionError>,
) {
    let Some(mut timer) = timer else { return };
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

/// Despawns all lightyear client entities (connected or pending) when returning
/// to the connect screen.  This ensures no stale client entity persists across
/// sessions, which would cause duplicate `Connect` triggers and server confusion
/// on the next connection attempt.
fn disconnect_client(
    mut commands: Commands,
    query: Query<Entity, With<PendingClient>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
        info!("disconnect_client: despawned client entity {:?}", entity);
    }
}

// ─── WASM-only: hostname WebTransport support ────────────────────────────────

/// Derive a SocketAddr suitable for the netcode connect-token from an
/// address string that may be either IP:port or hostname:port.
///
/// If the string is already a valid SocketAddr we use it directly.
/// Otherwise we extract the port (defaulting to 7778) and return
/// `0.0.0.0:<port>` as a dummy.  The lightyear_netcode server does not
/// validate the server_addresses field in the connect token (that check is
/// commented out), so any SocketAddr is accepted.
#[cfg(target_arch = "wasm32")]
fn wasm_netcode_addr(addr_str: &str) -> SocketAddr {
    if let Ok(sa) = addr_str.parse::<SocketAddr>() {
        return sa;
    }
    let port = addr_str
        .rfind(':')
        .and_then(|i| addr_str[i + 1..].parse::<u16>().ok())
        .unwrap_or(7778);
    SocketAddr::new(std::net::Ipv4Addr::UNSPECIFIED.into(), port)
}

/// Build a `ClientConfig` (= `xwt_web::WebTransportOptions`) for WASM.
///
/// `cert_hex` should be the hex-encoded SHA-256 digest of the server's
/// self-signed certificate, or empty when using a CA-signed certificate.
#[cfg(target_arch = "wasm32")]
fn build_wt_client_config(cert_hex: &str) -> Result<WtClientConfig, String> {
    use aeronet_webtransport::xwt_web::{CertificateHash, HashAlgorithm};

    let hashes = if cert_hex.is_empty() {
        vec![]
    } else {
        let bytes = hex_decode(cert_hex)?;
        vec![CertificateHash {
            algorithm: HashAlgorithm::Sha256,
            value: bytes,
        }]
    };
    Ok(WtClientConfig {
        server_certificate_hashes: hashes,
        ..Default::default()
    })
}

#[cfg(target_arch = "wasm32")]
fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err(format!("Odd-length hex string: \"{}\"", s));
    }
    s.as_bytes()
        .chunks(2)
        .map(|pair| {
            let hi = hex_nibble(pair[0])?;
            let lo = hex_nibble(pair[1])?;
            Ok((hi << 4) | lo)
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
fn hex_nibble(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("Invalid hex byte: 0x{:02x}", b)),
    }
}

/// Observer that handles `LinkStart` for entities carrying `WebTransportHostname`.
///
/// This mirrors `lightyear_webtransport::client::WebTransportClientPlugin::link`
/// but builds the connection URL directly from the stored string, so hostnames
/// are supported in addition to raw IP addresses.
#[cfg(target_arch = "wasm32")]
fn wt_hostname_link(
    trigger: On<LinkStart>,
    query: Query<(Entity, &WebTransportHostname), (Without<Linking>, Without<Linked>)>,
    mut commands: Commands,
) {
    let Ok((entity, wt)) = query.get(trigger.entity) else {
        return;
    };
    let url = wt.url.clone();
    let cert_digest = wt.cert_digest.clone();

    commands.queue(move |world: &mut World| {
        let config = match build_wt_client_config(&cert_digest) {
            Ok(c) => c,
            Err(e) => {
                error!("WebTransport config error: {}", e);
                return;
            }
        };
        let mut aeronet_entity = world.spawn((
            AeronetLinkOf(entity),
            Name::from("WebTransportClient"),
        ));
        WebTransportClient::connect(config, url).apply(aeronet_entity);
    });
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

    let mut sent = false;
    for mut sender in client_query.iter_mut() {
        sender.send::<GameChannel>(msg.clone());
        sent = true;
        warn!("[NET] Sent JoinMsg: id={} username='{}'", msg.client_id, msg.username);
    }
    if !sent {
        warn!("[NET] send_join_msg: query returned nothing — MessageSender<JoinMsg> not found!");
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
            &mut MessageReceiver<MapUrlMsg>,
        ),
        (With<Client>, With<Connected>),
    >,
    mut joined_events: MessageWriter<RemotePlayerJoined>,
    mut left_events: MessageWriter<RemotePlayerLeft>,
    mut moved_events: MessageWriter<RemotePlayerMoved>,
    mut damaged_events: MessageWriter<LocalPlayerDamaged>,
    mut kill_events: MessageWriter<RemoteKillEvent>,
    mut map_url_events: MessageWriter<LoadMapFromUrl>,
) {
    for (
        mut join_rx,
        mut leave_rx,
        mut pos_rx,
        mut damage_rx,
        mut kill_rx,
        mut map_url_rx,
    ) in client_query.iter_mut()
    {
        // PlayerJoinMsg
        for msg in join_rx.receive() {
            warn!(
                "[NET] Received PlayerJoinMsg: id={} username='{}' color={:?}",
                msg.client_id, msg.username, msg.color
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

        // MapUrlMsg — server is telling us which map to load.
        for msg in map_url_rx.receive() {
            info!("[MAP] Server sent map URL: {}", msg.url);
            map_url_events.write(LoadMapFromUrl(msg.url));
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

/// Appends `:7777` when the user omitted the port, handling all address forms:
///   IPv4 bare:       `1.2.3.4`        → `1.2.3.4:7777`
///   IPv4 with port:  `1.2.3.4:7777`   → unchanged
///   IPv6 bare:       `::1`            → `[::1]:7777`
///   IPv6 bracketed:  `[::1]`          → `[::1]:7777`
///   IPv6 with port:  `[::1]:7777`     → unchanged
///   Hostname bare:   `example.com`    → `example.com:7777`
///   Hostname:port:   `example.com:80` → unchanged
fn append_default_port(addr: &str) -> String {
    // Bracketed IPv6 — either [::1]:port (has port) or [::1] (no port).
    if addr.starts_with('[') {
        return if addr.contains("]:") {
            addr.to_string()
        } else {
            format!("{}:7777", addr)
        };
    }
    // Bare IPv6 address (contains colons and parses as IpAddr) → add brackets + port.
    if addr.contains(':') {
        if addr.parse::<std::net::IpAddr>().is_ok() {
            return format!("[{}]:7777", addr);
        }
        // host:port — already has a port separator.
        return addr.to_string();
    }
    // IPv4 or hostname without port.
    format!("{}:7777", addr)
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_addr(addr_str: &str) -> Result<SocketAddr, String> {
    addr_str
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| format!("No address resolved for '{}'", addr_str))
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
    // Bind to the same IP family as the server address so the OS doesn't reject
    // the sendto with EINVAL (can't send IPv6 packets from an IPv4 socket).
    let local_addr = if server_addr.is_ipv6() {
        SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0)
    } else {
        SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0)
    };
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

/// WASM: WebTransport transport using a full URL string.
///
/// Unlike the native path, this uses `WebTransportHostname` instead of
/// `WebTransportClientIo` + `PeerAddr(SocketAddr)`.  A custom observer
/// (`wt_hostname_link`) handles `LinkStart` for these entities and builds the
/// WebTransport connection from `url` directly, so both IP addresses and
/// hostnames are supported.
///
/// `RSG_CERT_DIGEST` must be set at compile time to the SHA-256 fingerprint
/// of the server's self-signed cert (hex, no colons).  Leave it unset when
/// the server uses a CA-signed certificate.
#[cfg(target_arch = "wasm32")]
fn spawn_transport(
    commands: &mut Commands,
    addr_str: &str,
    netcode_client: NetcodeClient,
    _profile: &PlayerProfile,
) {
    use lightyear::prelude::client::Connect;

    let cert_digest = option_env!("RSG_CERT_DIGEST")
        .unwrap_or("")
        .to_string();

    let url = format!("https://{addr_str}");

    let entity = commands.spawn((
        Name::new("GameClient"),
        PendingClient,
        WebTransportHostname { url: url.clone(), cert_digest },
        netcode_client,
    )).id();
    commands.trigger(Connect { entity });
    info!("Using WebTransport → {}", url);
}
