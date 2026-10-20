//! `OrnamentalFan`: block for every third attack in a turn.

use sts2_core::{CardKind, EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `CardsVar(3)`.
const CARDS: u8 = 3;

/// `BlockVar(4, Unpowered)`.
const BLOCK: Number = 4;

/// `AttacksPlayedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    attacks_played_this_turn: u8,
}

/// `AfterCardPlayed` counts each attack its owner plays, and every third
/// gives its block. `BeforeSideTurnStart` on its owner's side starts
/// the count again, and `AfterCombatEnd` leaves it.
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
            if state.attacks_played_this_turn.is_multiple_of(CARDS) {
                asked.push(HookEffect::GainBlock(BLOCK));
            }
        }
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        } => state.attacks_played_this_turn = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the attacks counted towards the next block.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.attacks_played_this_turn % CARDS)
}
