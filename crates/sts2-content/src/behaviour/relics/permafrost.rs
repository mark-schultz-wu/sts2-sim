//! `Permafrost`: block for the first power a fight plays.

use crate::vars::RelicVars as _;
use sts2_core::{CardTable, EventHook, HookEffect};

/// `_activatedThisCombat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    activated_this_combat: bool,
}

/// The two hooks this relic answers.
///
/// `AfterRoomEntered` clears the flag for a room the run fights in.
/// `AfterCardPlayed` pays out once, for the first power card its owner plays.
///
/// # Panics
///
/// A card that works its own type out from the fight, which no card played
/// under this relic does yet.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::RoomEntered { room, .. } if crate::behaviour::relics::combat_room(room) => {
            state.activated_this_combat = false;
        }
        EventHook::CardPlayed { card, .. } if !state.activated_this_combat => {
            let power = card
                .power()
                .unwrap_or_else(|| unimplemented!("a card that works its own type out"));
            match power {
                false => {}
                true => {
                    state.activated_this_combat = true;
                    asked.push(HookEffect::GainBlock(V::Relic::PERMAFROST_BLOCK))
                }
            }
        }
        _ => {}
    }
}
