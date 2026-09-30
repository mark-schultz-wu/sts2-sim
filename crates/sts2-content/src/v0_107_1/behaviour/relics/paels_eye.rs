//! `PaelsEye`: a turn with no card played exhausts the hand and is followed
//! by another, once a fight.

use sts2_core::{DecisionHook, EventHook, HookEffect, HookResponse, SideKinds as _};

use super::super::super::V0_107_1 as V;

/// Its two fields, `UsedThisCombat` and `WasOwnerPartOfLastPlayerTurn`.
///
/// The second reads whether the owner took part in the last player turn,
/// which a fight here always has it do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    used_this_combat: bool,
    was_owner_part_of_last_player_turn: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            used_this_combat: false,
            was_owner_part_of_last_player_turn: true,
        }
    }
}

impl State {
    /// `!UsedThisCombat && !AnyCardsPlayedThisTurn() &&
    /// WasOwnerPartOfLastPlayerTurn`, with `plays` the player's finished
    /// plays this turn that no auto-play made.
    ///
    /// `AnyCardsPlayedThisTurn` also answers yes on turn one for an owner
    /// holding Whispering Earring, which this port has not reached.
    fn takes_the_turn(&self, plays: u8) -> bool {
        !self.used_this_combat && plays == 0 && self.was_owner_part_of_last_player_turn
    }
}

/// `AfterObtained` and `AfterSideTurnStart` note that the owner takes part
/// in the player's turn. `BeforeSideTurnEndEarly` exhausts the hand of a
/// turn that earns the extra one. `AfterTakingExtraTurn` spends it for the
/// fight, and `AfterCombatEnd` gives it back. `BeforeCardPlayed` sets only
/// the relic's `Status`, which the game shows and nothing reads.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let player = <V as sts2_core::ContentVersion>::CombatSide::PLAYER;
    match hook {
        EventHook::Obtained { .. } => state.was_owner_part_of_last_player_turn = true,
        EventHook::SideTurnStart { side, .. } if side == player && !state.used_this_combat => {
            state.was_owner_part_of_last_player_turn = true;
        }
        EventHook::BeforeSideTurnEndEarly { side, plays } if side == player => {
            if state.takes_the_turn(plays) {
                asked.push(HookEffect::ExhaustHand);
            }
        }
        EventHook::TakingExtraTurn => state.used_this_combat = true,
        EventHook::CombatEnd { .. } => state.used_this_combat = false,
        EventHook::SideTurnStart { .. }
        | EventHook::BeforeSideTurnEndEarly { .. }
        | EventHook::CardPlaying { .. } => {}
        hook => unimplemented!("what PaelsEye does for {hook:?}"),
    }
}

/// `ShouldTakeExtraTurn`.
pub(super) fn decide_hook(state: &State, hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldTakeExtraTurn { plays } => {
            HookResponse::Decision(state.takes_the_turn(plays))
        }
        hook => unimplemented!("what PaelsEye decides for {hook:?}"),
    }
}
