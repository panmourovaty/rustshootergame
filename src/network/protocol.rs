/// Shared network protocol definitions used by both the server and client.
///
/// Shared protocol — compatible with lightyear 0.26 / Bevy 0.18.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

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

// ─── Messages ────────────────────────────────────────────────────────────────

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
