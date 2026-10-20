//! `GlassEye`: five card rewards as it is picked up, one of each rarity it
//! names.

use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`: `RewardsCmd.OfferCustom` over the five.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferRewards(V::GLASS_EYE_REWARDS));
    }
}

/// `REWARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// A `CardReward` of three over the character's pool at one rarity, for each
        /// of Common, Common, Uncommon, Uncommon and Rare. The pool's hooks reach
        /// each.
        const GLASS_EYE_REWARDS: &'static [sts2_core::CustomReward<$build>] = &[
            sts2_core::CustomReward::CardsOf {
                pool: None,
                rarity: sts2_core::RewardRarity::Common,
                count: 3,
                upgrade_roll: false,
                no_pool_modifications: false,
            },
            sts2_core::CustomReward::CardsOf {
                pool: None,
                rarity: sts2_core::RewardRarity::Common,
                count: 3,
                upgrade_roll: false,
                no_pool_modifications: false,
            },
            sts2_core::CustomReward::CardsOf {
                pool: None,
                rarity: sts2_core::RewardRarity::Uncommon,
                count: 3,
                upgrade_roll: false,
                no_pool_modifications: false,
            },
            sts2_core::CustomReward::CardsOf {
                pool: None,
                rarity: sts2_core::RewardRarity::Uncommon,
                count: 3,
                upgrade_roll: false,
                no_pool_modifications: false,
            },
            sts2_core::CustomReward::CardsOf {
                pool: None,
                rarity: sts2_core::RewardRarity::Rare,
                count: 3,
                upgrade_roll: false,
                no_pool_modifications: false,
            },
        ];
    };
}

pub(crate) use lists;
