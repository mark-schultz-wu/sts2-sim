//! `MercuryHourglass`: damage to every enemy as each of its owner's turns
//! starts.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterPlayerTurnStart`, which is `CreatureCmd.Damage` on every enemy the
/// player can hit. A relic's owner is the player, and a fight here has one,
/// so the player starting a turn is always its own.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::PlayerTurnStart { .. } = hook {
        asked.push(HookEffect::DamageOpponents(
            V::Relic::MERCURY_HOURGLASS_DAMAGE,
        ));
    }
}
