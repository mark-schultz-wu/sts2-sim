//! `PumpkinCandle`: an energy each turn while it burns, for a few fights at
//! a time, and a rest site option that rekindles it.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::ids::RestSiteOptionIds as _;

/// `Rekindle`'s five fights, which is `CombatCount`.
const KINDLE: Number = 5;

/// `KindleCount`, its one `[SavedProperty]`: the fights it still burns for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    kindle_count: Number,
}

/// `AfterObtained` and the Kindle option's `OnSelect` rekindle it.
/// `AfterCombatEnd` burns a fight off. `TryModifyRestSiteOptions` offers
/// Kindle.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } | EventHook::RestOptionTaken => state.kindle_count += KINDLE,
        EventHook::CombatEnd { .. } => state.kindle_count = (state.kindle_count - 1).max(0),
        EventHook::RestSiteOptions => {
            asked.push(HookEffect::OfferRestOption(V::RestSiteOption::KINDLE))
        }
        _ => {}
    }
}

/// `ModifyMaxEnergy`: its energy while it burns.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &State,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::MaxEnergy { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::MaxEnergy { .. } if state.kindle_count > 0 => {
            Modify::Added(V::Relic::PUMPKIN_CANDLE_ENERGY)
        }
        ModifyHook::MaxEnergy { .. } => Modify::Nothing,
        hook => unimplemented!("what PumpkinCandle does for {hook:?}"),
    }
}

/// `DisplayAmount`: the fights it still burns for.
pub(super) fn display_amount(state: &State) -> Number {
    state.kindle_count
}
