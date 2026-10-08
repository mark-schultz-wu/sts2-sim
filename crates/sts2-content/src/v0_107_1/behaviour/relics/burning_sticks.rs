//! `BurningSticks`: the first Skill exhausted in each fight, cloned into
//! hand.

use sts2_core::{CardKind, EventHook, HookEffect, Rules as _};

use super::super::super::V0_107_1 as V;

/// `WasUsedThisCombat`, which each combat room entered clears.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    used: bool,
}

/// `AfterRoomEntered` in a combat room, and `AfterCombatEnd`, clear the
/// flag. `AfterCardExhausted` of a Skill, with the flag clear, adds a clone
/// of that card to its owner's hand and sets it. A relic's owner is the
/// player, and a fight here has one, so every card is its owner's.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::RoomEntered { room, .. } if super::combat_room(room) => state.used = false,
        EventHook::CombatEnd { .. } => state.used = false,
        EventHook::CardExhausted { card, id, .. }
            if !state.used && V::card_kind(card) == Some(CardKind::Skill) =>
        {
            asked.push(HookEffect::CloneIntoHand {
                card: id,
                count: 1,
                writes: &[],
            });
            state.used = true;
        }
        _ => {}
    }
}
