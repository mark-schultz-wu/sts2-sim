//! `PollinousCore`: two more cards in every fourth hand, counted across fights.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `TurnsSeen`, a saved property, so a fight's end leaves it as it is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    pub(crate) turns_seen: Number,
}

/// `BeforeSideTurnStart` counts its owner's turns, and `AfterModifyingHandDraw`
/// starts the count again once the hand it changed is worked out. Its
/// `AfterCombatEnd` only resets the counter's look.
/// Each acts where its owner takes part in the turn.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
) {
    match hook {
        EventHook::BeforeSideTurnStart { participants, .. } if participants.contains(owner) => {
            state.turns_seen += 1
        }
        // The game hands this hook only to a listener that changed the hand,
        // which this one did on the paying turn alone.
        EventHook::HandDrawModified if state.turns_seen == V::Relic::POLLINOUS_CORE_TURNS => {
            state.turns_seen = 0
        }
        _ => {}
    }
}

/// `ModifyHandDraw`: its cards on the paying turn.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &State,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::HandDraw { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::HandDraw { .. } if state.turns_seen == V::Relic::POLLINOUS_CORE_TURNS => {
            Modify::Added(V::Relic::POLLINOUS_CORE_CARDS)
        }
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what PollinousCore does for {hook:?}"),
    }
}

/// `DisplayAmount`, which is the count.
pub(super) fn display_amount(state: &State) -> Number {
    state.turns_seen
}
