//! `VelvetChoker`: one more energy each turn, and no more than six cards
//! played a turn.

use crate::vars::RelicVars as _;
use sts2_core::{DecisionHook, EventHook, HookResponse, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `_cardsPlayedThisTurn`, every play of its owner's, auto-plays and replays
/// among them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    played: u8,
}

/// `AfterCardPlayed` counts each play. `BeforeSideTurnStart` for its owner's
/// side, `AfterRoomEntered` for a combat room and `AfterCombatEnd` set the
/// count back to none.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
) {
    match hook {
        EventHook::CardPlayed { player, .. } if player == owner => {
            state.played = state.played.saturating_add(1)
        }
        EventHook::RoomEntered { room, .. } if crate::behaviour::relics::combat_room(room) => {
            state.played = 0
        }
        EventHook::BeforeSideTurnStart { participants, .. } if participants.contains(owner) => {
            state.played = 0
        }
        EventHook::CombatEnd { .. } => state.played = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the plays made this turn.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.played)
}

/// `ShouldPlay`: no, once its owner has played six cards this turn.
pub(super) fn decide_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &State,
    hook: DecisionHook<V>,
) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldPlay { player, .. } if player != owner => HookResponse::Decision(true),
        DecisionHook::ShouldPlay { .. } => {
            HookResponse::Decision(state.played < (V::Relic::VELVET_CHOKER_CARDS as u8))
        }
        hook => unimplemented!("what VelvetChoker decides for {hook:?}"),
    }
}

/// `ModifyMaxEnergy`: its energy, for its owner.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::MaxEnergy { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::MaxEnergy { .. } => Modify::Added(V::Relic::VELVET_CHOKER_ENERGY),
        hook => unimplemented!("what VelvetChoker does for {hook:?}"),
    }
}
