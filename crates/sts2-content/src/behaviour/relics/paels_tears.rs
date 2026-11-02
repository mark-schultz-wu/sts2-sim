//! `PaelsTears`: energy on a turn after one that ended with some left.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `HadLeftoverEnergy`, which it keeps between one turn's end and the next
/// turn's start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    had_leftover: bool,
}

/// Each turn hook acts where its owner takes part.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        // `BeforeSideTurnEnd`: whether the player's energy is above nothing.
        EventHook::BeforeSideTurnEnd {
            participants,
            energy,
            ..
        } if participants.contains(owner) => state.had_leftover = energy > 0,
        // `AfterSideTurnStart`: `PlayerCmd.GainEnergy` after a turn that
        // ended with energy left.
        EventHook::SideTurnStart { participants, .. }
            if participants.contains(owner) && state.had_leftover =>
        {
            asked.push(HookEffect::GainEnergy(V::Relic::PAELS_TEARS_ENERGY))
        }
        EventHook::CombatEnd { .. } => state.had_leftover = false,
        _ => {}
    }
}
