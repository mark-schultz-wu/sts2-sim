//! `WhisperingEarring`: one more energy each turn, and Vakuu plays the first
//! turn's hand.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `maxCardsToPlay`.
const PLAYS: u8 = 13;

/// `AfterAutoPrePlayPhaseEnteredLate` on its owner's first turn: the hand's
/// playable cards, paid for and auto-played, thirteen at most.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::AutoPrePlayPhaseLate { player, turn } = hook
        && player == owner
        && turn <= 1
    {
        asked.push(HookEffect::AutoPlayHand { left: PLAYS });
    }
}

/// `ModifyMaxEnergy`: its energy, on every turn.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::MaxEnergy { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::MaxEnergy { .. } => Modify::Added(V::Relic::WHISPERING_EARRING_ENERGY),
        hook => unimplemented!("what WhisperingEarring does for {hook:?}"),
    }
}
