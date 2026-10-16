//! `Shuriken`: Strength for every third attack in a turn.

use sts2_core::{CardKind, EventHook, HookEffect, PowerAmount};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;
use crate::ids::PowerIds as _;

/// `CardsVar(3)`.
const CARDS: u8 = 3;

/// `PowerVar<StrengthPower>(1)`.
const STRENGTH: PowerAmount<Number> = PowerAmount::Counted(1);

/// `AttacksPlayedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    attacks_played_this_turn: u8,
}

/// `AfterCardPlayed` counts each attack its owner plays, and every third
/// gives its Strength. `BeforeSideTurnStart` on its owner's side and
/// `AfterCombatEnd` start the count again.
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
                asked.push(HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, STRENGTH));
            }
        }
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        }
        | EventHook::CombatEnd { .. } => state.attacks_played_this_turn = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the attacks counted towards the next Strength.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.attacks_played_this_turn % CARDS)
}
