//! `MercuryHourglass`: damage to every enemy as each of its owner's turns
//! starts.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterPlayerTurnStart`, which is `CreatureCmd.Damage` on every enemy the
/// player can hit, on its owner's turn.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::PlayerTurnStart { player, .. } = hook
        && player != owner
    {
        return;
    }
    if let EventHook::PlayerTurnStart { .. } = hook {
        asked.push(HookEffect::DamageOpponents(
            V::Relic::MERCURY_HOURGLASS_DAMAGE,
        ));
    }
}
