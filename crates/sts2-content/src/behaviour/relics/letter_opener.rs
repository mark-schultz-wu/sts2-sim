//! `LetterOpener`: damage to every enemy for every third skill in a turn.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect};

use crate::behaviour::relics::Number;

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
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlayed { card, player, .. }
            if player == owner
                && <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Skill) =>
        {
            state.skills_played_this_turn += 1;
            if state
                .skills_played_this_turn
                .is_multiple_of(V::Relic::LETTER_OPENER_CARDS as u8)
            {
                asked.push(HookEffect::DamageOpponents(V::Relic::LETTER_OPENER_DAMAGE));
            }
        }
        EventHook::SideTurnStart {
            participants, turn, ..
        } if participants.contains(owner) && turn != 1 => state.skills_played_this_turn = 0,
        EventHook::CombatStarting { .. } => state.skills_played_this_turn = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the skills counted towards the next damage.
pub(super) fn display_amount<V: crate::Build>(state: &State) -> Number {
    Number::from(state.skills_played_this_turn % (V::Relic::LETTER_OPENER_CARDS as u8))
}
