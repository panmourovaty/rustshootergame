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
use lightyear::prelude::{Connected, LocalAddr, NetworkTarget};
use lightyear::prelude::{MessageReceiver, MessageSender};
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use aeronet_webtransport::wtransport::Identity;
use lightyear::prelude::server::WebTransportServerIo;

use super::protocol::{
    GameChannel, HitMsg, JoinMsg, KillNotifyMsg, MapUrlMsg, PlayerJoinMsg, PlayerLeaveMsg,
    PosChannel, PosUpdateMsg, ProtocolPlugin, RelayedPosMsg, TakeDamageMsg, PROTOCOL_ID,
};

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

        // Register messages and channels AFTER ServerPlugins.
        app.add_plugins(ProtocolPlugin);
        app.add_plugins(GameServerPlugin);

        app.insert_resource(ServerPorts {
            udp: self.port,
            web: self.web_port,
        });
        app.init_resource::<MapUrl>();
        app.add_systems(Startup, spawn_server_entities);
        app.add_systems(Update, log_client_connections);

        info!(
            "ServerNetworkPlugin registered (UDP :{}, WebTransport :{})",
            self.port, self.web_port
        );
    }
}

// ─── Game server plugin ──────────────────────────────────────────────────────

/// Separate plugin that handles the game-level server logic (player tracking,
/// damage, relaying positions, etc.).
pub struct GameServerPlugin;

impl Plugin for GameServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_join_msg,
                relay_positions,
                handle_hit_msg,
            ),
        );

        // Observer for disconnections.
        app.add_observer(handle_disconnect);
    }
}

// ─── Components ─────────────────────────────────────────────────────────────

/// Server-side state for each connected client, stored on the `ClientOf` entity.
#[derive(Component, Clone)]
pub struct ServerPlayerInfo {
    pub client_id: u64,
    pub username: String,
    pub hp: f32,
}

// ─── Resources ──────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct ServerPorts {
    pub udp: u16,
    pub web: u16,
}

/// The map archive URL set via `--map-url`.  `None` means clients use the
/// built-in placeholder map.
#[derive(Resource, Default)]
pub struct MapUrl(pub Option<String>);

// ─── Colour palette ──────────────────────────────────────────────────────────

/// 8 distinct colours assigned by `client_id % 8`.
fn color_for_id(client_id: u64) -> [f32; 3] {
    const PALETTE: [[f32; 3]; 8] = [
        [1.0, 0.2, 0.2], // red
        [0.2, 0.6, 1.0], // blue
        [0.2, 1.0, 0.2], // green
        [1.0, 1.0, 0.2], // yellow
        [1.0, 0.5, 0.0], // orange
        [0.8, 0.2, 1.0], // purple
        [0.2, 1.0, 0.9], // cyan
        [1.0, 0.4, 0.7], // pink
    ];
    PALETTE[(client_id % 8) as usize]
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
    let udp_entity = commands.spawn((
        Name::new("GameServerUdp"),
        ServerUdpIo::default(),
        LocalAddr(udp_addr),
        NetcodeServer::new(netcode_config.clone()),
    )).id();
    commands.trigger(Start { entity: udp_entity });
    info!("UDP listener on {}", udp_addr);

    // ── WebTransport listener (browser clients) ───────────────────────────────
    let wt_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), ports.web);

    let identity = Identity::self_signed(["localhost", "127.0.0.1", "::1"])
        .expect("failed to generate self-signed TLS certificate");

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

    let wt_entity = commands.spawn((
        Name::new("GameServerWebTransport"),
        WebTransportServerIo {
            certificate: identity,
        },
        LocalAddr(wt_addr),
        NetcodeServer::new(netcode_config),
    )).id();
    commands.trigger(Start { entity: wt_entity });
}

fn log_client_connections(query: Query<Entity, Added<Connected>>) {
    for entity in query.iter() {
        info!("Client connected: entity {:?}", entity);
    }
}

/// Reads `JoinMsg` from newly connected clients, stores `ServerPlayerInfo`,
/// then broadcasts `PlayerJoinMsg` to all existing clients and sends each
/// existing player's info to the newcomer.  Also sends `MapUrlMsg` when a
/// map URL has been configured via `--map-url`.
fn handle_join_msg(
    mut commands: Commands,
    map_url: Res<MapUrl>,
    // Split into two separate queries to avoid aliased-mutability conflicts.
    mut join_rx_query: Query<(Entity, &mut MessageReceiver<JoinMsg>), With<ClientOf>>,
    existing_info_query: Query<&ServerPlayerInfo, With<ClientOf>>,
    mut announce_senders: Query<&mut MessageSender<PlayerJoinMsg>, With<ClientOf>>,
    mut map_url_senders: Query<&mut MessageSender<MapUrlMsg>, With<ClientOf>>,
) {
    // Phase 1: collect all join messages (drains the receivers).
    let mut joins: Vec<(Entity, JoinMsg)> = Vec::new();
    for (entity, mut receiver) in join_rx_query.iter_mut() {
        for msg in receiver.receive() {
            joins.push((entity, msg));
        }
    }

    if joins.is_empty() {
        return;
    }

    // Phase 2: snapshot the already-known players before we insert new info.
    let existing_snapshot: Vec<(u64, String)> = existing_info_query
        .iter()
        .map(|i| (i.client_id, i.username.clone()))
        .collect();

    for (_new_entity, join_msg) in joins {
        let color = color_for_id(join_msg.client_id);

        let connected_count = announce_senders.iter().count();
        warn!(
            "[SERVER] Player '{}' (id={}) joined — broadcasting to {} connected clients",
            join_msg.username, join_msg.client_id, connected_count
        );

        // Insert player info component on the client entity.
        commands.entity(_new_entity).insert(ServerPlayerInfo {
            client_id: join_msg.client_id,
            username: join_msg.username.clone(),
            hp: 100.0,
        });

        let announce = PlayerJoinMsg {
            client_id: join_msg.client_id,
            username: join_msg.username.clone(),
            color,
        };

        // Broadcast the new player's info to ALL connected clients.
        let mut broadcast_count = 0usize;
        for mut sender in announce_senders.iter_mut() {
            sender.send::<GameChannel>(announce.clone());
            broadcast_count += 1;
        }
        warn!("[SERVER] Broadcast PlayerJoinMsg for id={} to {} senders", join_msg.client_id, broadcast_count);

        // Send each pre-existing player's info to the newcomer so their
        // lobby/scoreboard is populated immediately.
        for (existing_id, ref existing_name) in &existing_snapshot {
            if *existing_id == join_msg.client_id {
                continue;
            }
            let existing_msg = PlayerJoinMsg {
                client_id: *existing_id,
                username: existing_name.clone(),
                color: color_for_id(*existing_id),
            };
            // The newcomer's sender is in announce_senders; send only to them.
            if let Ok(mut sender) = announce_senders.get_mut(_new_entity) {
                sender.send::<GameChannel>(existing_msg);
                warn!("[SERVER] Sent existing player id={} info to newcomer id={}", existing_id, join_msg.client_id);
            }
        }

        // If a map URL is configured, send it to the newcomer so they can
        // download and load the dynamic map.
        if let Some(url) = &map_url.0 {
            if let Ok(mut sender) = map_url_senders.get_mut(_new_entity) {
                sender.send::<GameChannel>(MapUrlMsg { url: url.clone() });
                info!("[SERVER] Sent MapUrlMsg to id={}: {}", join_msg.client_id, url);
            }
        }
    }
}

/// Observer triggered when a `Connected` component is removed (client
/// disconnects).  Broadcasts `PlayerLeaveMsg` to remaining clients.
fn handle_disconnect(
    trigger: On<Remove, Connected>,
    info_query: Query<&ServerPlayerInfo>,
    mut senders: Query<&mut MessageSender<PlayerLeaveMsg>, With<ClientOf>>,
) {
    let disconnected_entity = trigger.event().entity;

    let Ok(player_info) = info_query.get(disconnected_entity) else {
        return; // Client disconnected before sending JoinMsg.
    };

    bevy::log::info!(
        "Player '{}' (id={}) disconnected",
        player_info.username, player_info.client_id
    );

    let leave_msg = PlayerLeaveMsg {
        client_id: player_info.client_id,
    };

    for mut sender in senders.iter_mut() {
        sender.send::<GameChannel>(leave_msg.clone());
    }
}

/// Reads position updates from each client and relays them to all other clients.
fn relay_positions(
    mut readers: Query<
        (&ServerPlayerInfo, &mut MessageReceiver<PosUpdateMsg>),
        With<ClientOf>,
    >,
    mut senders: Query<&mut MessageSender<RelayedPosMsg>, With<ClientOf>>,
) {
    // Collect all incoming position updates.
    let mut relays: Vec<(u64, PosUpdateMsg)> = Vec::new();
    for (player_info, mut receiver) in readers.iter_mut() {
        for msg in receiver.receive() {
            relays.push((player_info.client_id, msg));
        }
    }

    // Re-broadcast each update to all clients (clients skip their own id).
    for (sender_client_id, pos_msg) in relays {
        let relay = RelayedPosMsg {
            client_id: sender_client_id,
            pos: pos_msg.pos,
            yaw: pos_msg.yaw,
        };
        for mut sender in senders.iter_mut() {
            sender.send::<PosChannel>(relay.clone());
        }
    }
}

/// Processes hit reports from clients: decrements HP on the server, sends
/// `TakeDamageMsg` to the victim, and broadcasts `KillNotifyMsg` on death.
///
/// Uses separate queries to avoid aliased-mutability conflicts:
///   - `hit_rx_query` reads HitMsg receivers (needs &ServerPlayerInfo read-only).
///   - `victim_query` mutably updates HP and sends damage messages.
///   - `kill_senders` broadcasts kill notifications.
fn handle_hit_msg(
    mut hit_rx_query: Query<&mut MessageReceiver<HitMsg>, With<ClientOf>>,
    mut victim_query: Query<
        (Entity, &mut ServerPlayerInfo, &mut MessageSender<TakeDamageMsg>),
        With<ClientOf>,
    >,
    mut kill_senders: Query<&mut MessageSender<KillNotifyMsg>, With<ClientOf>>,
) {
    // Collect hit messages from all client receivers.
    let mut hits: Vec<HitMsg> = Vec::new();
    for mut receiver in hit_rx_query.iter_mut() {
        for msg in receiver.receive() {
            hits.push(msg);
        }
    }

    for hit in hits {
        // Find victim entity by client_id.
        let victim_entity = victim_query
            .iter()
            .find(|(_, player_info, _)| player_info.client_id == hit.victim_id)
            .map(|(e, _, _)| e);

        let Some(victim_entity) = victim_entity else {
            continue;
        };

        if let Ok((_e, mut player_info, mut damage_sender)) = victim_query.get_mut(victim_entity) {
            player_info.hp = (player_info.hp - hit.damage).max(0.0);
            let new_hp = player_info.hp;

            // Tell the victim their new HP.
            damage_sender.send::<GameChannel>(TakeDamageMsg { new_hp });

            bevy::log::info!(
                "Player {} hit player {} for {:.0} dmg → {:.0} HP",
                hit.killer_id, hit.victim_id, hit.damage, new_hp
            );

            if new_hp <= 0.0 {
                // Reset HP for the next life.
                player_info.hp = 100.0;

                let kill_msg = KillNotifyMsg {
                    killer_id: hit.killer_id,
                    victim_id: hit.victim_id,
                };

                for mut sender in kill_senders.iter_mut() {
                    sender.send::<GameChannel>(kill_msg.clone());
                }

                bevy::log::info!(
                    "Kill! Player {} killed player {}.",
                    hit.killer_id, hit.victim_id
                );
            }
        }
    }
}
