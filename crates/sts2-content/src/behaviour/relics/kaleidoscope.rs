//! `Kaleidoscope`: card rewards from other characters' pools.

use sts2_core::{EventHook, HookEffect};

/// `Cards`: the card rewards it offers.
const REWARDS: usize = 2;

/// The other characters' pools each reward draws one card from.
const POOLS: usize = 3;

/// `AfterObtained`: `RewardsCmd.OfferCustom` over two card rewards, each one
/// card from each of three other characters' pools.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferOtherPoolCards {
            rewards: REWARDS,
            pools: POOLS,
        });
    }
}
