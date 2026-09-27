//! `LostCoffer`: a card reward and a potion as it is picked up.

use sts2_core::{CustomReward, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// A three-card reward from the character's pool, then a potion.
const REWARDS: &[CustomReward<V>] = &[CustomReward::CharacterCards(3), CustomReward::Potion];

/// `AfterObtained`: `RewardsCmd.OfferCustom` with a `CardReward` of three
/// over its owner's pool at `RegularEncounter` odds, and a `PotionReward`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained = hook {
        asked.push(HookEffect::OfferRewards(REWARDS));
    }
}
