//! `TuningFork`: block for every tenth skill, counted across fights.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `SkillsPlayed`, which it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    skills_played: u8,
}

/// `AfterCardPlayed` counts each skill its owner plays. The tenth gives its
/// block and takes ten off the count.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { card, player, .. } = hook
        && player == owner
        && <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Skill)
    {
        state.skills_played += 1;
        if state.skills_played >= (V::Relic::TUNING_FORK_CARDS as u8) {
            asked.push(HookEffect::GainBlock(V::Relic::TUNING_FORK_BLOCK));
            state.skills_played -= V::Relic::TUNING_FORK_CARDS as u8;
        }
    }
}

/// `DisplayAmount`: the skills counted towards the next block.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.skills_played)
}
