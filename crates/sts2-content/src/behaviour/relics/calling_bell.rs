//! `CallingBell`: a curse, and a relic of each rarity to pick from.

use sts2_core::{EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `AfterObtained`: `AddCurseToDeck<CurseOfTheBell>`, then
/// `RewardsCmd.OfferCustom` over the three.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::AddToDeck(V::Card::CURSE_OF_THE_BELL));
        asked.push(HookEffect::OfferRewards(V::CALLING_BELL_REWARDS));
    }
}

/// `REWARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `RelicReward` at Common, Uncommon and Rare.
        const CALLING_BELL_REWARDS: &'static [sts2_core::CustomReward<$build>] = &[
            sts2_core::CustomReward::Relic(crate::generated::relics::RelicRarity::Common),
            sts2_core::CustomReward::Relic(crate::generated::relics::RelicRarity::Uncommon),
            sts2_core::CustomReward::Relic(crate::generated::relics::RelicRarity::Rare),
        ];
    };
}

pub(crate) use lists;
