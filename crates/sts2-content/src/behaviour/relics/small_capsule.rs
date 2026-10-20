//! `SmallCapsule`: a relic reward as it is picked up.

use sts2_core::{CustomReward, EventHook, HookEffect};

/// `AfterObtained`: `RewardsCmd.OfferCustom` with a `RelicReward(player)`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferRewards(&[CustomReward::RolledRelic]));
    }
}
