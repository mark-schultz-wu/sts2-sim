//! `TuningFork`: block for every tenth skill, counted across fights.

use sts2_core::{CardKind, EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `CardsVar(10)`.
const CARDS: u8 = 10;

/// `BlockVar(7, Unpowered)`.
const BLOCK: Number = 7;

/// `SkillsPlayed`, which it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    skills_played: u8,
}

/// `AfterCardPlayed` counts each skill its owner plays. The tenth gives its
/// block and takes ten off the count.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { card, .. } = hook
        && <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Skill)
    {
        state.skills_played += 1;
        if state.skills_played >= CARDS {
            asked.push(HookEffect::GainBlock(BLOCK));
            state.skills_played -= CARDS;
        }
    }
}

/// `DisplayAmount`: the skills counted towards the next block.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.skills_played)
}
