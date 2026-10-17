//! `Kunai`: Dexterity for every third attack in a turn.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect, PowerAmount};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;
use crate::ids::PowerIds as _;

/// `AttacksPlayedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    attacks_played_this_turn: u8,
}

/// `AfterCardPlayed` counts each attack its owner plays, and every third
/// gives its Dexterity. `BeforeSideTurnStart` on its owner's side and
/// `AfterCombatEnd` start the count again.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlayed { card, .. }
            if <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Attack) =>
        {
            state.attacks_played_this_turn += 1;
            if state
                .attacks_played_this_turn
                .is_multiple_of(V::Relic::KUNAI_CARDS as u8)
            {
                asked.push(HookEffect::ApplyToOwner(
                    V::Power::DEXTERITY_POWER,
                    PowerAmount::Counted(V::Relic::KUNAI_DEXTERITY_POWER),
                ));
            }
        }
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        }
        | EventHook::CombatEnd { .. } => state.attacks_played_this_turn = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the attacks counted towards the next Dexterity.
pub(super) fn display_amount<V: crate::Build>(state: &State) -> Number {
    Number::from(state.attacks_played_this_turn % (V::Relic::KUNAI_CARDS as u8))
}
