//! `Orrery`: five card rewards as it is picked up.

use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`: `RewardsCmd.OfferCustom` over the five.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferRewards(V::ORRERY_REWARDS));
    }
}

/// `REWARD`, `REWARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// One `CardReward` of three over the character's pool at
        /// `RegularEncounter` odds.
        const ORRERY_REWARD: sts2_core::CustomReward<$build> =
            sts2_core::CustomReward::CharacterCards {
                count: 3,
                upgrade_roll: true,
            };
        /// Its `CardsVar(5)` rewards.
        const ORRERY_REWARDS: &'static [sts2_core::CustomReward<$build>] =
            &[<$build as crate::build::Lists>::ORRERY_REWARD; 5];
    };
}

pub(crate) use lists;
