//! `Pocketwatch`: the cards played in a turn, and the draw the next one gets.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

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
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
) {
    match hook {
        EventHook::CardPlayed { player, .. } if player == owner => state.played_this_turn += 1,
        EventHook::BeforeSideTurnStart { participants, .. } if participants.contains(owner) => {
            state.played_last_turn = state.played_this_turn;
            state.played_this_turn = 0;
        }
        EventHook::CombatEnd { .. } => *state = State::default(),
        _ => {}
    }
}

/// `ModifyHandDraw`: three more cards for a turn that followed a quiet one.
///
/// The first turn of a fight follows nothing and draws its printed hand.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &State,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::HandDraw { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::HandDraw { turn, .. }
            if turn > 1 && state.played_last_turn <= V::Relic::POCKETWATCH_CARD_THRESHOLD =>
        {
            Modify::Added(V::Relic::POCKETWATCH_CARDS)
        }
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what Pocketwatch does for {hook:?}"),
    }
}

/// `DisplayAmount`, which is this turn's count.
pub(super) fn display_amount(state: &State) -> Number {
    state.played_this_turn
}
