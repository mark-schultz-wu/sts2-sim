//! `LetterOpener`: damage to every enemy for every third skill in a turn.

use sts2_core::{CardKind, EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `CardsVar(3)`.
const CARDS: u8 = 3;

/// `DamageVar(5, Unpowered)`.
const DAMAGE: Number = 5;

/// `SkillsPlayedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    skills_played_this_turn: u8,
}

/// `AfterCardPlayed` counts each skill its owner plays, and every third
/// deals its damage to every hittable enemy. `BeforeCombatStart` starts the
/// count again, and so does `AfterSideTurnStart` on its owner's side past
/// the first turn. `AfterCombatEnd` leaves it.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlayed { card, .. }
            if <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Skill) =>
        {
            state.skills_played_this_turn += 1;
            if state.skills_played_this_turn.is_multiple_of(CARDS) {
                asked.push(HookEffect::DamageOpponents(DAMAGE));
            }
        }
        EventHook::SideTurnStart {
            side: CombatSide::Player,
            turn,
            ..
        } if turn != 1 => state.skills_played_this_turn = 0,
        EventHook::CombatStarting { .. } => state.skills_played_this_turn = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the skills counted towards the next damage.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.skills_played_this_turn % CARDS)
}
