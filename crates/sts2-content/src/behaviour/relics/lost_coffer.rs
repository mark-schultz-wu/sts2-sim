//! `LostCoffer`: a card reward and a potion as it is picked up.

use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`: `RewardsCmd.OfferCustom` with a `CardReward` of three
/// over its owner's pool at `RegularEncounter` odds, and a `PotionReward`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferRewards(V::LOST_COFFER_REWARDS));
    }
}

/// `REWARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// A three-card reward from the character's pool, then a potion.
        const LOST_COFFER_REWARDS: &'static [sts2_core::CustomReward<$build>] = &[
            sts2_core::CustomReward::CharacterCards {
                count: 3,
                upgrade_roll: true,
            },
            sts2_core::CustomReward::Potion,
        ];
    };
}

pub(crate) use lists;
