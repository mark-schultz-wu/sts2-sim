//! `Kaleidoscope`: card rewards from other characters' pools.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `Cards`: the card rewards it offers.
const REWARDS: usize = 2;

/// The other characters' pools each reward draws one card from.
const POOLS: usize = 3;

/// `AfterObtained`: `RewardsCmd.OfferCustom` over two card rewards, each one
/// card from each of three other characters' pools.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferOtherPoolCards {
            rewards: REWARDS,
            pools: POOLS,
        });
    }
}
