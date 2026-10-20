//! `PollinousCore`: two more cards in every fourth hand, counted across fights.

use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `CardsVar(2)`.
const CARDS: Number = 2;

/// `DynamicVar("Turns", 4)`: the turn count that pays.
const TURNS: Number = 4;

/// `TurnsSeen`, a saved property, so a fight's end leaves it as it is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    turns_seen: Number,
}

/// `BeforeSideTurnStart` counts its owner's turns, and `AfterModifyingHandDraw`
/// starts the count again once the hand it changed is worked out. Its
/// `AfterCombatEnd` only resets the counter's look.
///
/// A relic's owner is the player, and a fight here has one, so the player's
/// side holds its owner.
pub(super) fn event_hook<V: crate::Build>(state: &mut State, hook: EventHook<V>) {
    match hook {
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        } => state.turns_seen += 1,
        // The game hands this hook only to a listener that changed the hand,
        // which this one did on the paying turn alone.
        EventHook::HandDrawModified if state.turns_seen == TURNS => state.turns_seen = 0,
        _ => {}
    }
}

/// `ModifyHandDraw`: its cards on the paying turn.
pub(super) fn modify_hook<V: crate::Build>(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HandDraw { .. } if state.turns_seen == TURNS => Modify::Added(CARDS),
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what PollinousCore does for {hook:?}"),
    }
}

/// `DisplayAmount`, which is the count.
pub(super) fn display_amount(state: &State) -> Number {
    state.turns_seen
}
