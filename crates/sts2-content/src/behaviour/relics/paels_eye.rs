//! `PaelsEye`: a turn with no card played exhausts the hand and is followed
//! by another, once a fight.

use crate::ids::RelicIds as _;
use sts2_core::{DecisionHook, EventHook, HookEffect, HookResponse, SideKinds as _};

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
    fn takes_the_turn<V: crate::Build>(&self, belt: &[V::Relic], plays: u8, turn: Count) -> bool {
        !self.used_this_combat
            && !any_cards_played_this_turn::<V>(belt, plays, turn)
            && self.was_owner_part_of_last_player_turn
    }
}

type Count = u32;

/// `AnyCardsPlayedThisTurn`: yes on the owner's first turn while it holds
/// Whispering Earring, which plays that turn for it. Otherwise whether it
/// finished a play no auto-play made.
fn any_cards_played_this_turn<V: crate::Build>(belt: &[V::Relic], plays: u8, turn: Count) -> bool {
    (turn == Count::from(1u8) && belt.contains(&V::Relic::WHISPERING_EARRING)) || plays > 0
}

/// `AfterObtained` and `AfterSideTurnStart` note that the owner takes part
/// in the player's turn. `BeforeSideTurnEndEarly` exhausts the hand of a
/// turn that earns the extra one. `AfterTakingExtraTurn` spends it for the
/// fight, and `AfterCombatEnd` gives it back. `BeforeCardPlayed` sets only
/// the relic's `Status`, which the game shows and nothing reads.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    belt: &[V::Relic],
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let player = <V as sts2_core::ContentVersion>::CombatSide::PLAYER;
    match hook {
        EventHook::Obtained { .. } => state.was_owner_part_of_last_player_turn = true,
        EventHook::SideTurnStart { side, .. } if side == player && !state.used_this_combat => {
            state.was_owner_part_of_last_player_turn = true;
        }
        EventHook::BeforeSideTurnEndEarly { side, plays, turn } if side == player => {
            if state.takes_the_turn::<V>(belt, plays, turn) {
                asked.push(HookEffect::ExhaustHand);
            }
        }
        EventHook::TakingExtraTurn { player } if player == owner => state.used_this_combat = true,
        EventHook::CombatEnd { .. } => state.used_this_combat = false,
        EventHook::SideTurnStart { .. }
        | EventHook::BeforeSideTurnEndEarly { .. }
        | EventHook::TakingExtraTurn { .. }
        | EventHook::CardPlaying { .. } => {}
        hook => unimplemented!("what PaelsEye does for {hook:?}"),
    }
}

/// `ShouldTakeExtraTurn`.
pub(super) fn decide_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &State,
    belt: &[V::Relic],
    hook: DecisionHook<V>,
) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldTakeExtraTurn {
            player,
            plays,
            turn,
        } => {
            HookResponse::Decision(player == owner && state.takes_the_turn::<V>(belt, plays, turn))
        }
        hook => unimplemented!("what PaelsEye decides for {hook:?}"),
    }
}
