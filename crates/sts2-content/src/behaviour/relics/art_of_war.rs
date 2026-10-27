//! `ArtOfWar`: energy on a turn after one its owner played no Attack in.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect, SideKinds as _};

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
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::EnergyReset { player, .. } = hook
        && player != owner
    {
        return;
    }
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
                asked.push(HookEffect::GainEnergy(V::Relic::ART_OF_WAR_ENERGY));
            }
            *state = State::default();
        }
        EventHook::CombatEnd { .. } => *state = State::default(),
        _ => {}
    }
}
