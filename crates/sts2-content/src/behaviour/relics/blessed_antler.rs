//! `BlessedAntler`: one more energy each turn, and three Dazed to open a
//! fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::ids::CardIds as _;

/// `BeforeHandDraw` on its owner's first turn:
/// `AddGeneratedCardsToCombat` of three Dazed into the draw pile, each at
/// `CardPilePosition.Random`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::BeforeHandDraw { turn: 1, .. } => asked.push(HookEffect::AddToDrawAtRandom {
            card: V::Card::DAZED,
            count: V::Relic::BLESSED_ANTLER_CARDS,
            by_owner: true,
        }),
        EventHook::BeforeHandDraw { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what BlessedAntler does for {hook:?}"),
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
        ModifyHook::MaxEnergy { .. } => Modify::Added(V::Relic::BLESSED_ANTLER_ENERGY),
        hook => unimplemented!("what BlessedAntler does for {hook:?}"),
    }
}
