//! `Bread`: two energy fewer on the first turn of a fight, and one more on
//! every turn after.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `AfterSideTurnStart`: its energy lost on its owner's first turn. A
/// relic's owner is the player, and a fight here has one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn: 1,
        ..
    } = hook
    {
        asked.push(HookEffect::LoseEnergy(V::Relic::BREAD_LOSE_ENERGY));
    }
}

/// `ModifyMaxEnergy`: its energy, save on its owner's first turn.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::MaxEnergy { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::MaxEnergy { turn: 1, .. } => Modify::Nothing,
        ModifyHook::MaxEnergy { .. } => Modify::Added(V::Relic::BREAD_GAIN_ENERGY),
        hook => unimplemented!("what Bread does for {hook:?}"),
    }
}
