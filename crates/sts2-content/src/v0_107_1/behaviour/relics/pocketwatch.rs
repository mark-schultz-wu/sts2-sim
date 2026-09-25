//! `Pocketwatch`: the cards played in a turn, and the draw the next one gets.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `DynamicVar("CardThreshold", 3)`: the most cards a turn plays and is still
/// paid for.
const CARD_THRESHOLD: Number = 3;

/// `CardsVar(3)`: what the next turn draws on top of its hand.
const CARDS: Number = 3;

/// `_cardsPlayedThisTurn` and `_cardsPlayedLastTurn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    played_this_turn: Number,
    played_last_turn: Number,
}

/// The three hooks this relic writes its own fields in.
///
/// `AfterCardPlayed` counts, `BeforeSideTurnStart` moves this turn's count to
/// last turn's before anything is drawn, and `AfterCombatEnd` forgets both.
/// `AfterSideTurnStart` only refreshes the counter on screen, so nothing here
/// answers it.
pub(super) fn event_hook(state: &mut State, hook: EventHook<V>) {
    match hook {
        EventHook::CardPlayed { .. } => state.played_this_turn += 1,
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        } => {
            state.played_last_turn = state.played_this_turn;
            state.played_this_turn = 0;
        }
        EventHook::CombatEnd => *state = State::default(),
        _ => {}
    }
}

/// `ModifyHandDraw`: three more cards for a turn that followed a quiet one.
///
/// The first turn of a fight follows nothing and draws its printed hand.
pub(super) fn modify_hook(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        // A relic's owner is the player, and a fight here has one, so the
        // player drawing is always its own.
        ModifyHook::HandDraw { turn, .. }
            if turn > 1 && state.played_last_turn <= CARD_THRESHOLD =>
        {
            Modify::Added(CARDS)
        }
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what Pocketwatch does for {hook:?}"),
    }
}

/// `DisplayAmount`, which is this turn's count.
pub(super) fn display_amount(state: &State) -> Number {
    state.played_this_turn
}
