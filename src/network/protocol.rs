/// Shared network protocol definitions used by both the server and client.
///
/// Shared protocol — compatible with lightyear 0.26 / Bevy 0.18.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use lightyear::prelude::*;

// ─── Constants ───────────────────────────────────────────────────────────────

pub const PROTOCOL_ID: u64 = 0xDEAD_BEEF_C0DE_0001;
pub const SERVER_PORT: u16 = 7777;

// ─── Components replicated over the network ───────────────────────────────────

/// A stable numeric identifier assigned to every networked player.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct NetworkId(pub u64);

/// The colour tint used to distinguish remote players on screen.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PlayerNetColor(pub [f32; 3]);

/// Health value synchronised from server → clients.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct NetworkedHealth(pub f32);

// ─── Input struct sent from client → server ──────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerInput {
    /// Packed movement bits: bit 0 = forward, 1 = back, 2 = left, 3 = right,
    /// 4 = jump, 5 = shoot.
    pub buttons: u8,
    /// Camera yaw in radians.
    pub yaw: f32,
    /// Camera pitch in radians.
    pub pitch: f32,
}

impl PlayerInput {
    pub fn forward(&self) -> bool {
        self.buttons & (1 << 0) != 0
    }
    pub fn back(&self) -> bool {
        self.buttons & (1 << 1) != 0
    }
    pub fn left(&self) -> bool {
        self.buttons & (1 << 2) != 0
    }
    pub fn right(&self) -> bool {
        self.buttons & (1 << 3) != 0
    }
    pub fn jump(&self) -> bool {
        self.buttons & (1 << 4) != 0
    }
    pub fn shoot(&self) -> bool {
        self.buttons & (1 << 5) != 0
    }
}

// ─── Legacy messages (kept for compatibility) ─────────────────────────────────

/// Sent server → all clients to announce a kill.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KillMessage {
    pub killer_id: u64,
    pub victim_id: u64,
}

/// Sent server → joining client with their assigned network id.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WelcomeMessage {
    pub your_id: u64,
}

// ─── Channels ────────────────────────────────────────────────────────────────

/// Reliable ordered channel — used for game state messages (joins, leaves,
/// kills, damage).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameChannel;

/// Unreliable channel — used for high-frequency position updates.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PosChannel;

// ─── Client → Server messages ─────────────────────────────────────────────────

/// Client announces itself to the server when entering Playing state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinMsg {
    pub client_id: u64,
    pub username: String,
}

/// Client sends its position/orientation every ~50 ms.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PosUpdateMsg {
    pub pos: [f32; 3],
    pub yaw: f32,
}

/// Client reports that it hit a remote player.
/// The server validates HP and issues `TakeDamageMsg` / `KillNotifyMsg`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HitMsg {
    pub killer_id: u64,
    pub victim_id: u64,
    pub damage: f32,
}

// ─── Server → Client messages ─────────────────────────────────────────────────

/// Server broadcasts to all existing clients when a new player connects, and
/// to the new client for each player already in the session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerJoinMsg {
    pub client_id: u64,
    pub username: String,
    pub color: [f32; 3],
}

/// Server broadcasts when a player disconnects.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerLeaveMsg {
    pub client_id: u64,
}

/// Server relays position updates to all clients except the originator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelayedPosMsg {
    pub client_id: u64,
    pub pos: [f32; 3],
    pub yaw: f32,
}

/// Server tells a specific client to update its local HP display.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TakeDamageMsg {
    pub new_hp: f32,
}

/// Server broadcasts a confirmed kill to all clients.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KillNotifyMsg {
    pub killer_id: u64,
    pub victim_id: u64,
}

// ─── Protocol Plugin ─────────────────────────────────────────────────────────

pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // ── Channels ──────────────────────────────────────────────────────────
        app.add_channel::<GameChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);

        app.add_channel::<PosChannel>(ChannelSettings {
            mode: ChannelMode::UnorderedUnreliable,
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);

        // ── Client → Server messages ──────────────────────────────────────────
        app.register_message::<JoinMsg>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<PosUpdateMsg>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<HitMsg>()
            .add_direction(NetworkDirection::ClientToServer);

        // ── Server → Client messages ──────────────────────────────────────────
        app.register_message::<PlayerJoinMsg>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<PlayerLeaveMsg>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<RelayedPosMsg>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<TakeDamageMsg>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<KillNotifyMsg>()
            .add_direction(NetworkDirection::ServerToClient);
    }
}
