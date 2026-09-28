//! `WongosMysteryTicket`: three relics with the fifth fight's rewards.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::Number;

/// `combatsToActivate`.
const COMBATS: Number = 5;

/// `relicCount`.
const RELICS: usize = 3;

/// `CombatsFinished` and `GaveRelic`, the two `[SavedProperty]`s it declares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    combats_finished: Number,
    gave_relic: bool,
}

/// `AfterCombatEnd` counts every fight. `TryModifyRewards` adds a relic
/// reward for each of `relicCount` to a fight's rewards, once five have
/// finished, and `AfterModifyingRewards` then marks it spent.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CombatEnd { .. } => state.combats_finished += 1,
        EventHook::ModifyRewards {
            room: Some(RoomType::Monster | RoomType::Elite | RoomType::Boss),
            ..
        } if !state.gave_relic && state.combats_finished >= COMBATS => {
            state.gave_relic = true;
            asked.push(HookEffect::AddRelicRewards(RELICS));
        }
        _ => {}
    }
}

/// `DisplayAmount`, which is the fights still to finish.
pub(super) fn display_amount(state: &State) -> Number {
    COMBATS - state.combats_finished
}

/// `IsUsedUp`, which is `GaveRelic`.
pub(super) fn used_up(state: &State) -> bool {
    state.gave_relic
}
