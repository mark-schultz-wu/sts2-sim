//! `SmallCapsule`: a relic reward as it is picked up.

use sts2_core::{CustomReward, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `AfterObtained`: `RewardsCmd.OfferCustom` with a `RelicReward(player)`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferRewards(&[CustomReward::RolledRelic]));
    }
}
