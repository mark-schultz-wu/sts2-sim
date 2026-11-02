//! `BloodSoakedRose`: one more energy each turn, and Enthralled in the deck.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::ids::CardIds as _;

/// `AfterObtained`: `CardPileCmd.AddCurseToDeck<Enthralled>`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => asked.push(HookEffect::AddToDeck(V::Card::ENTHRALLED)),
        hook => unimplemented!("what BloodSoakedRose does for {hook:?}"),
    }
}

/// `ModifyMaxEnergy`: its energy, for its owner.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::MaxEnergy { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::MaxEnergy { .. } => Modify::Added(V::Relic::BLOOD_SOAKED_ROSE_ENERGY),
        hook => unimplemented!("what BloodSoakedRose does for {hook:?}"),
    }
}
