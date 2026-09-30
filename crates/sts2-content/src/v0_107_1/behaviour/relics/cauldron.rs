//! `Cauldron`: five potion rewards as it is picked up.

use sts2_core::{CustomReward, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// Its `DynamicVar("Potions", 5)` rewards, each `PotionReward(player)`.
const REWARDS: [CustomReward<V>; 5] = [CustomReward::Potion; 5];

/// `AfterObtained`: `RewardsCmd.OfferCustom` over the five.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferRewards(&REWARDS));
    }
}
