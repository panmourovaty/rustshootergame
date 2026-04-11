use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

// ─── Action enum ───────────────────────────────────────────────────────────

/// Gameplay actions that can be bound to keyboard / mouse inputs.
///
/// Each variant maps to one or more physical inputs through an
/// `InputMap<PlayerAction>` component on the `LocalPlayer` entity.
/// The settings screen can rebind these at runtime by mutating that
/// component.
///
/// Implements `Serialize`/`Deserialize` so that lightyear can
/// network the action state via `lightyear_inputs_leafwing`.
#[derive(
    Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect, Serialize, Deserialize,
)]
pub enum PlayerAction {
    // ── Movement ──────────────────────────────────────────────────────────
    MoveForward,
    MoveBack,
    MoveLeft,
    MoveRight,
    Jump,

    // ── Combat ────────────────────────────────────────────────────────────
    Shoot,
    Reload,

    // ── UI / meta ─────────────────────────────────────────────────────────
    Pause,
    Scoreboard,
}

// ─── Plugin ────────────────────────────────────────────────────────────────

/// Registers the appropriate input plugin for the current build:
///
/// - **With networking**: uses `lightyear_inputs_leafwing::InputPlugin`, which
///   adds both the Leafwing `InputManagerPlugin` and the lightyear
///   client/server input networking systems that automatically capture,
///   serialise, and replay `ActionState<PlayerAction>` each tick.
///
/// - **Without networking**: uses the plain Leafwing `InputManagerPlugin` so
///   that offline / test builds still get action-state tracking.
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PlayerAction>();

        #[cfg(any(feature = "networking", feature = "web"))]
        {
            // lightyear_inputs_leafwing::InputPlugin adds:
            //   - leafwing_input_manager::InputManagerPlugin<PlayerAction>
            //   - lightyear client input capture / server replay systems
            // It also registers ActionState<PlayerAction> and InputBuffer types
            // for reflection and serialisation.
            app.add_plugins(
                lightyear::prelude::input::leafwing::InputPlugin::<PlayerAction>::default(),
            );
        }

        #[cfg(not(any(feature = "networking", feature = "web")))]
        {
            app.add_plugins(InputManagerPlugin::<PlayerAction>::default());
        }
    }
}