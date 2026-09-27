//! `TinyMailbox`: two potion rewards after each rest site's heal.

use sts2_core::{CustomReward, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `TryModifyRestSiteHealRewards`'s two `PotionReward(player)`.
const REWARDS: [CustomReward<V>; 2] = [CustomReward::Potion, CustomReward::Potion];

/// `TryModifyRestSiteHealRewards`: two potion rewards for its owner, an
/// event's heal included. A run here has one player.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::RestSiteHealRewards { .. } = hook {
        asked.push(HookEffect::OfferRewards(&REWARDS));
    }
}
