//! `BoneTea`: the hand it upgrades, and the one combat it has to do it in.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

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

/// The opening hand, upgraded once: in v0.107.1's `AfterSideTurnStart`, and
/// in v0.111.0's `AfterPlayerTurnStart`. Each build's class overrides one of
/// the two, so it hears only that one.
///
/// The game returns early for a used-up relic, for a turn after the first, and
/// for a turn that is not its owner's. Each of those is an arm this does not
/// match.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::PlayerTurnStart { player, .. } = hook
        && player != owner
    {
        return;
    }
    match hook {
        EventHook::SideTurnStart {
            participants,
            turn: 1,
            ..
        } if participants.contains(owner) && !state.used_up() => {
            state.combats_left -= 1;
            asked.push(HookEffect::UpgradeHand)
        }
        EventHook::PlayerTurnStart { turn: 1, .. } if !state.used_up() => {
            state.combats_left -= 1;
            asked.push(HookEffect::UpgradeHand)
        }
        _ => {}
    }
}

/// `DisplayAmount`, which is `Math.Max(0, CombatsLeft)`.
pub(super) fn display_amount(state: &State) -> Number {
    state.combats_left.max(0)
}

/// `IsUsedUp`, once no fight is left.
pub(super) fn used_up(state: &State) -> bool {
    state.combats_left <= 0
}
