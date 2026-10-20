//! `PrismaticGem`: an energy each turn, and card rewards drawn from every
//! character's pool.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `ModifyMaxEnergy`: one more.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { .. } => Modify::Added(ENERGY),
        hook => unimplemented!("what PrismaticGem does for {hook:?}"),
    }
}

/// `ModifyCardRewardCreationOptions`: `UnlockState.CharacterCardPools`, which
/// is every character's with every character unlocked, joined to the
/// reward's own. The run asks only for a `CardReward` whose options take a
/// change to their pools, and whose pool is not colorless alone.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardRewardCreationOptions { .. } => {
            asked.push(HookEffect::AddCharacterPools(V::ALL_CHARACTERS));
        }
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what PrismaticGem does for {hook:?}"),
    }
}
