//! `BurningSticks`: the first Skill exhausted in each fight, cloned into
//! hand.

use sts2_core::{CardKind, EventHook, HookEffect};

/// `WasUsedThisCombat`, which each combat room entered clears.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    used: bool,
}

/// `AfterRoomEntered` in a combat room, and `AfterCombatEnd`, clear the
/// flag. `AfterCardExhausted` of one of its owner's Skills, with the flag
/// clear, adds a clone of that card to its owner's hand and sets it.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::RoomEntered { room, .. } if crate::behaviour::relics::combat_room(room) => {
            state.used = false
        }
        EventHook::CombatEnd { .. } => state.used = false,
        EventHook::CardExhausted {
            card, id, player, ..
        } if player == owner && !state.used && V::card_kind(card) == Some(CardKind::Skill) => {
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
