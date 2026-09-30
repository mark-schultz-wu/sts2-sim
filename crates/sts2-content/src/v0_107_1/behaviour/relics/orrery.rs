//! `Orrery`: five card rewards as it is picked up.

use sts2_core::{CustomReward, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// One `CardReward` of three over the character's pool at
/// `RegularEncounter` odds.
const REWARD: CustomReward<V> = CustomReward::CharacterCards {
    count: 3,
    upgrade_roll: true,
};

/// Its `CardsVar(5)` rewards.
const REWARDS: [CustomReward<V>; 5] = [REWARD; 5];

/// `AfterObtained`: `RewardsCmd.OfferCustom` over the five.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferRewards(&REWARDS));
    }
}
