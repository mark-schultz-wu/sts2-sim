//! `BlessedAntler`: one more energy each turn, and three Dazed to open a
//! fight.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, StatusCard};
use super::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `CardsVar(3)`.
const DAZED: Number = 3;

/// `BeforeHandDraw` on its owner's first turn:
/// `AddGeneratedCardsToCombat` of three Dazed into the draw pile, each at
/// `CardPilePosition.Random`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::BeforeHandDraw { turn: 1, .. } => asked.push(HookEffect::AddToDrawAtRandom {
            card: Cards::Status(StatusCard::Dazed),
            count: DAZED,
        }),
        EventHook::BeforeHandDraw { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what BlessedAntler does for {hook:?}"),
    }
}

/// `ModifyMaxEnergy`: its energy, for its owner.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { .. } => Modify::Added(ENERGY),
        hook => unimplemented!("what BlessedAntler does for {hook:?}"),
    }
}
