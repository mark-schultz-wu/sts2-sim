//! `MercuryHourglass`: damage to every enemy as each of its owner's turns
//! starts.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `DamageVar(3, ValueProp.Unpowered)`.
const DAMAGE: Number = 3;

/// `AfterPlayerTurnStart`, which is `CreatureCmd.Damage` on every enemy the
/// player can hit. A relic's owner is the player, and a fight here has one,
/// so the player starting a turn is always its own.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::PlayerTurnStart { .. } = hook {
        asked.push(HookEffect::DamageOpponents(DAMAGE));
    }
}
