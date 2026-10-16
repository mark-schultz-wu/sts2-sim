//! `WhisperingEarring`: one more energy each turn, and Vakuu plays the first
//! turn's hand.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `maxCardsToPlay`.
const PLAYS: u8 = 13;

/// `AfterAutoPrePlayPhaseEnteredLate` on its owner's first turn: the hand's
/// playable cards, paid for and auto-played, thirteen at most. A relic's
/// owner is the player, and a fight here has one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::AutoPrePlayPhaseLate { turn } = hook
        && turn <= 1
    {
        asked.push(HookEffect::AutoPlayHand { left: PLAYS });
    }
}

/// `ModifyMaxEnergy`: its energy, on every turn.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { .. } => Modify::Added(ENERGY),
        hook => unimplemented!("what WhisperingEarring does for {hook:?}"),
    }
}
