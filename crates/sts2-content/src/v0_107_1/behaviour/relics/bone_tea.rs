//! `BoneTea`: the hand it upgrades, and the one combat it has to do it in.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `_combatsLeft`, which the relic is picked up with.
const COMBATS: Number = 1;

/// `CombatsLeft`, the one `[SavedProperty]` this relic declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    combats_left: Number,
}

impl Default for State {
    fn default() -> Self {
        Self {
            combats_left: COMBATS,
        }
    }
}

impl State {
    /// `IsUsedUp`, which is what disables the relic on screen.
    fn used_up(self) -> bool {
        self.combats_left <= 0
    }
}

/// `AfterSideTurnStart`: the opening hand, upgraded once.
///
/// The game returns early for a used-up relic, for a turn after the first, and
/// for a side turn its owner is not in. Each of those is an arm this does not
/// match.
pub(super) fn event_hook(state: &mut State, hook: EventHook<V>) -> Vec<HookEffect<V>> {
    match hook {
        EventHook::SideTurnStart {
            side: CombatSide::Player,
            turn: 1,
        } if !state.used_up() => {
            state.combats_left -= 1;
            vec![HookEffect::UpgradeHand]
        }
        _ => Vec::new(),
    }
}

/// `DisplayAmount`, which is `Math.Max(0, CombatsLeft)`.
pub(super) fn display_amount(state: &State) -> Number {
    state.combats_left.max(0)
}
