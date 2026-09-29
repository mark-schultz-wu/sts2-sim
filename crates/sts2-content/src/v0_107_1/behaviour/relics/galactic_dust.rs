//! `GalacticDust`: block for every ten stars its owner spends.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `StarsVar(10)`.
const STARS: Number = 10;

/// `BlockVar(10, ValueProp.Unpowered)`.
const BLOCK: Number = 10;

/// `StarsSpent`, its one `[SavedProperty]`, which carries from one fight to
/// the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    stars_spent: Number,
}

/// `AfterStarsSpent` for its owner counts the stars. Once they reach its
/// `Stars`, it gains its `Block` for each whole `Stars`, unpowered, and
/// keeps the rest.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::StarsSpent { amount, .. } = hook {
        state.stars_spent += amount;
        if state.stars_spent >= STARS {
            asked.push(HookEffect::GainBlock(state.stars_spent / STARS * BLOCK));
            state.stars_spent %= STARS;
        }
    }
}

/// `DisplayAmount`: the stars counted toward the next payout.
/// `IsActivating` shows the whole count while the relic's visuals play,
/// which a run without a screen never does.
pub(super) fn display_amount(state: &State) -> Number {
    state.stars_spent % STARS
}
