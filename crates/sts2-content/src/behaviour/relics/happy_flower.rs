//! `HappyFlower`: an energy every third turn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `TurnsSeen`, its one `[SavedProperty]`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    turns_seen: Number,
}

/// `AfterSideTurnStart` on its owner's side counts the turn, and every third
/// gives its energy.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart { participants, .. } = hook
        && participants.contains(owner)
    {
        state.turns_seen = (state.turns_seen + 1) % V::Relic::HAPPY_FLOWER_TURNS;
        if state.turns_seen == 0 {
            asked.push(HookEffect::GainEnergy(V::Relic::HAPPY_FLOWER_ENERGY));
        }
    }
}

/// `DisplayAmount`: the turns seen. `IsActivating` shows the whole count
/// while the relic's visuals play, which a run without a screen never does.
pub(super) fn display_amount(state: &State) -> Number {
    state.turns_seen
}
