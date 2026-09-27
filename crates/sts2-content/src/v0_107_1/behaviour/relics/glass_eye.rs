//! `GlassEye`: five card rewards as it is picked up, one of each rarity it
//! names.

use sts2_core::{CustomReward, EventHook, HookEffect, RewardRarity};

use super::super::super::V0_107_1 as V;

/// A `CardReward` of three over the character's pool at one rarity, for each
/// of Common, Common, Uncommon, Uncommon and Rare.
const REWARDS: [CustomReward; 5] = [
    CustomReward::CardsOf {
        rarity: RewardRarity::Common,
        count: 3,
    },
    CustomReward::CardsOf {
        rarity: RewardRarity::Common,
        count: 3,
    },
    CustomReward::CardsOf {
        rarity: RewardRarity::Uncommon,
        count: 3,
    },
    CustomReward::CardsOf {
        rarity: RewardRarity::Uncommon,
        count: 3,
    },
    CustomReward::CardsOf {
        rarity: RewardRarity::Rare,
        count: 3,
    },
];

/// `AfterObtained`: `RewardsCmd.OfferCustom` over the five.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained = hook {
        asked.push(HookEffect::OfferRewards(&REWARDS));
    }
}
