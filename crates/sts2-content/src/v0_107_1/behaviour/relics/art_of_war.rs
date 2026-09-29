//! `ArtOfWar`: energy on a turn after one its owner played no Attack in.

use sts2_core::{CardKind, EventHook, HookEffect, Rules as _, SideKinds as _};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `AnyAttacksPlayedLastTurn` and `AnyAttacksPlayedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    last_turn: bool,
    this_turn: bool,
}

/// `AfterCardPlayed` of an Attack notes it for this turn, unless last turn
/// already had one. `AfterSideTurnEnd` for the player's side moves this
/// turn's note to last turn's. `AfterEnergyReset` past the first turn gives
/// its energy where last turn had no Attack, then clears both.
/// `AfterCombatEnd` clears both.
///
/// A relic's owner is the player, and a fight here has one, so every card
/// played is its owner's.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlayed { card, .. }
            if V::card_kind(card) == Some(CardKind::Attack) && !state.last_turn =>
        {
            state.this_turn = true;
        }
        EventHook::SideTurnEnd { side, .. }
            if side == <V as sts2_core::ContentVersion>::CombatSide::PLAYER =>
        {
            state.last_turn = state.this_turn;
            state.this_turn = false;
        }
        EventHook::EnergyReset { turn, .. } if turn > 1 => {
            if !state.last_turn {
                asked.push(HookEffect::GainEnergy(ENERGY));
            }
            *state = State::default();
        }
        EventHook::CombatEnd { .. } => *state = State::default(),
        _ => {}
    }
}
