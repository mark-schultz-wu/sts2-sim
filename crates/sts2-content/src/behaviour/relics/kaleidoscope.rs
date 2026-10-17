//! `Kaleidoscope`: card rewards from other characters' pools.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

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
            rewards: (V::Relic::KALEIDOSCOPE_CARDS as usize),
            pools: POOLS,
        });
    }
}
