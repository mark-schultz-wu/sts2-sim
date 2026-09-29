//! `CallingBell`: a curse, and a relic of each rarity to pick from.

use sts2_core::{CustomReward, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::super::super::generated::relics::RelicRarity;

/// `RelicReward` at Common, Uncommon and Rare.
const REWARDS: [CustomReward<V>; 3] = [
    CustomReward::Relic(RelicRarity::Common),
    CustomReward::Relic(RelicRarity::Uncommon),
    CustomReward::Relic(RelicRarity::Rare),
];

/// `AfterObtained`: `AddCurseToDeck<CurseOfTheBell>`, then
/// `RewardsCmd.OfferCustom` over the three.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::AddToDeck(Cards::Curse(
            CurseCard::CurseOfTheBell,
        )));
        asked.push(HookEffect::OfferRewards(&REWARDS));
    }
}
