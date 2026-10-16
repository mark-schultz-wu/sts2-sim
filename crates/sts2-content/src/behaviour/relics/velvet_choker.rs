//! `VelvetChoker`: one more energy each turn, and no more than six cards
//! played a turn.

use sts2_core::{DecisionHook, EventHook, HookResponse, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `CardsVar(6)`: the plays a turn allows.
const CARDS: u8 = 6;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `_cardsPlayedThisTurn`, every play of its owner's, auto-plays and replays
/// among them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    played: u8,
}

/// `AfterCardPlayed` counts each play. `BeforeSideTurnStart` for its owner's
/// side, `AfterRoomEntered` for a combat room and `AfterCombatEnd` set the
/// count back to none. A relic's owner is the player, and a fight here has
/// one, so every card is its owner's.
pub(super) fn event_hook<V: crate::Build>(state: &mut State, hook: EventHook<V>) {
    match hook {
        EventHook::CardPlayed { .. } => state.played = state.played.saturating_add(1),
        EventHook::RoomEntered { room, .. } if crate::behaviour::relics::combat_room(room) => {
            state.played = 0
        }
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        }
        | EventHook::CombatEnd { .. } => state.played = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the plays made this turn.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.played)
}

/// `ShouldPlay`: no, once its owner has played six cards this turn.
pub(super) fn decide_hook<V: crate::Build>(
    state: &State,
    hook: DecisionHook<V>,
) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldPlay { .. } => HookResponse::Decision(state.played < CARDS),
        hook => unimplemented!("what VelvetChoker decides for {hook:?}"),
    }
}

/// `ModifyMaxEnergy`: its energy, for its owner.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { .. } => Modify::Added(ENERGY),
        hook => unimplemented!("what VelvetChoker does for {hook:?}"),
    }
}
