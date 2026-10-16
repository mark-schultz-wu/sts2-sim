//! `Cauldron`: five potion rewards as it is picked up.

use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`: `RewardsCmd.OfferCustom` over the five.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferRewards(V::CAULDRON_REWARDS));
    }
}

/// `REWARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// Its `DynamicVar("Potions", 5)` rewards, each `PotionReward(player)`.
        const CAULDRON_REWARDS: &'static [sts2_core::CustomReward<$build>] =
            &[sts2_core::CustomReward::Potion; 5];
    };
}

pub(crate) use lists;
