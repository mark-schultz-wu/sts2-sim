//! `PrismaticGem`: an energy each turn, and card rewards drawn from every
//! character's pool.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyMaxEnergy`: one more.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::MaxEnergy { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::MaxEnergy { .. } => Modify::Added(V::Relic::PRISMATIC_GEM_ENERGY),
        hook => unimplemented!("what PrismaticGem does for {hook:?}"),
    }
}

/// `ModifyCardRewardCreationOptions`: `UnlockState.CharacterCardPools`, which
/// is every character's with every character unlocked, joined to the
/// reward's own. The run asks only for a `CardReward` whose options take a
/// change to their pools, and whose pool is not colorless alone.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardRewardCreationOptions { player, .. } if player == owner => {
            asked.push(HookEffect::AddCharacterPools(V::ALL_CHARACTERS));
        }
        EventHook::CardRewardCreationOptions { .. } => {}
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what PrismaticGem does for {hook:?}"),
    }
}
