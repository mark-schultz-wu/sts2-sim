//! `BloodSoakedRose`: one more energy each turn, and Enthralled in the deck.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `AfterObtained`: `CardPileCmd.AddCurseToDeck<Enthralled>`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::Obtained { .. } => {
            asked.push(HookEffect::AddToDeck(Cards::Curse(CurseCard::Enthralled)))
        }
        hook => unimplemented!("what BloodSoakedRose does for {hook:?}"),
    }
}

/// `ModifyMaxEnergy`: its energy, for its owner.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { .. } => Modify::Added(ENERGY),
        hook => unimplemented!("what BloodSoakedRose does for {hook:?}"),
    }
}
