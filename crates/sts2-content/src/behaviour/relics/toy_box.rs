//! `ToyBox`: its `Relics` of wax relics offered as it is picked up, one
//! melted every third fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `CombatsSeen`, which it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    combats_seen: u8,
}

/// `AfterObtained`: `RewardsCmd.OfferCustom` of a `RelicReward` for each of
/// its `Relics` off the front of the bag, each made wax.
/// `AfterCombatEnd`: one more fight seen, and each third melts the first wax
/// relic still whole.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => asked.push(HookEffect::OfferRewards(V::TOY_BOX_WAX_RELICS)),
        EventHook::CombatEnd { .. } => {
            state.combats_seen = (state.combats_seen.checked_add(1))
                .unwrap_or_else(|| unimplemented!("more fights than a byte counts"));
            if state
                .combats_seen
                .is_multiple_of(V::Relic::TOY_BOX_COMBATS as u8)
            {
                asked.push(HookEffect::MeltFirstWax);
            }
        }
        hook => unimplemented!("what ToyBox does for {hook:?}"),
    }
}

/// `IsUsedUp`: every wax relic's fights seen.
pub(crate) fn used_up<V: crate::Build>(state: &State) -> bool {
    usize::from(state.combats_seen)
        >= usize::from(V::Relic::TOY_BOX_COMBATS as u8) * V::TOY_BOX_WAX_RELICS.len()
}

/// `DisplayAmount`: the fights toward the next melt.
pub(super) fn display_amount<V: crate::Build>(state: &State) -> Number {
    Number::from(state.combats_seen % (V::Relic::TOY_BOX_COMBATS as u8))
}

/// `WAX_RELICS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `Relics`: the wax relics it offers.
        const TOY_BOX_WAX_RELICS: &'static [sts2_core::CustomReward<$build>] =
            &[sts2_core::CustomReward::WaxRelic;
                <<$build as sts2_core::ContentVersion>::Relic as crate::vars::RelicVars>::TOY_BOX_RELICS
                as usize];
    };
}

pub(crate) use lists;
#[cfg(test)]
mod tests {
    type V = crate::v0_107_1::V0_107_1;
    use crate::behaviour::relics::toy_box::*;

    /// Twelve fights, three for each wax relic, use the box up, and eleven
    /// do not.
    #[test]
    fn twelve_fights_use_the_box_up() {
        let seen = |combats_seen| State { combats_seen };
        assert!(!used_up::<V>(&seen(11)));
        assert!(used_up::<V>(&seen(12)));
    }
}
