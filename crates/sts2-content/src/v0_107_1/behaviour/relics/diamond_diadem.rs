//! `DiamondDiadem`: half damage from attacks after a turn of few cards.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `CardThreshold`, the most cards a turn can play and still count.
const THRESHOLD: u32 = 2;

/// `CardsPlayedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    cards_played: u32,
}

/// `AfterCardPlayed` counts each card its owner plays. `BeforeSideTurnEnd`
/// on its owner's side applies Diamond Diadem where the count stayed at the
/// threshold or under, then starts again. `AfterCombatEnd` starts again too.
/// A fight here has one player.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlayed { .. } => state.cards_played += 1,
        EventHook::BeforeSideTurnEnd {
            side: CombatSide::Player,
            ..
        } => {
            if state.cards_played <= THRESHOLD {
                asked.push(HookEffect::ApplyToOwner(
                    Power::DiamondDiademPower,
                    PowerAmount::Counted(1),
                ));
            }
            state.cards_played = 0;
        }
        EventHook::CombatEnd { .. } => state.cards_played = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the cards played this turn.
pub(super) fn display_amount(state: &State) -> Number {
    Number::try_from(state.cards_played).expect("a turn's plays fit a game number")
}
