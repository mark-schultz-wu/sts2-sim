//! `RoyalPoison`: damage as the first turn of a fight starts.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterPlayerTurnStart` on its owner's first turn: its damage to its
/// owner, from nobody.
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
    match hook {
        EventHook::PlayerTurnStart { turn, .. } if turn <= 1 => {
            asked.push(HookEffect::DamageOwnerInFight {
                amount: V::Relic::ROYAL_POISON_DAMAGE,
                unblockable: true,
                self_dealt: false,
            })
        }
        EventHook::PlayerTurnStart { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what RoyalPoison does for {hook:?}"),
    }
}
