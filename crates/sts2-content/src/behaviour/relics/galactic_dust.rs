//! `GalacticDust`: block for every ten stars its owner spends.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `StarsSpent`, its one `[SavedProperty]`, which carries from one fight to
/// the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    stars_spent: Number,
}

/// `AfterStarsSpent` for its owner counts the stars. Once they reach its
/// `Stars`, it gains its `Block` for each whole `Stars`, unpowered, and
/// keeps the rest.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::StarsSpent { player, .. } = hook
        && player != owner
    {
        return;
    }
    if let EventHook::StarsSpent { amount, .. } = hook {
        state.stars_spent += amount;
        if state.stars_spent >= V::Relic::GALACTIC_DUST_STARS {
            asked.push(HookEffect::GainBlock(
                state.stars_spent / V::Relic::GALACTIC_DUST_STARS * V::Relic::GALACTIC_DUST_BLOCK,
            ));
            state.stars_spent %= V::Relic::GALACTIC_DUST_STARS;
        }
    }
}

/// `DisplayAmount`: the stars counted toward the next payout.
/// `IsActivating` shows the whole count while the relic's visuals play,
/// which a run without a screen never does.
pub(super) fn display_amount<V: crate::Build>(state: &State) -> Number {
    state.stars_spent % V::Relic::GALACTIC_DUST_STARS
}
