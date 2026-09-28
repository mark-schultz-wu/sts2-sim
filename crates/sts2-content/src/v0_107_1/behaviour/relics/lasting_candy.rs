//! `LastingCandy`: a Power card offered beside every other fight's reward.

use sts2_core::{CardKind, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `CombatsSeen`, which it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    combats_seen: Number,
}

/// `AfterCombatEnd` counts the fight. `TryModifyCardRewardOptions`, for a
/// won fight's reward once an even number has ended, adds a Power card.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CombatEnd { .. } => state.combats_seen += 1,
        // `IsInTriggeringCombat`.
        EventHook::CardRewardOptions { encounter: true }
            if state.combats_seen > 0 && state.combats_seen % 2 == 0 =>
        {
            asked.push(HookEffect::AddCardRewardOption(CardKind::Power));
        }
        EventHook::CardRewardOptions { .. } | EventHook::Obtained => {}
        hook => unimplemented!("what LastingCandy does for {hook:?}"),
    }
}

/// `DisplayAmount`: the fights towards the next Power card.
pub(super) fn display_amount(state: &State) -> Number {
    state.combats_seen % 2
}
