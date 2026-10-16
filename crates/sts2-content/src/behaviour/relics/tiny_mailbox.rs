//! `TinyMailbox`: two potion rewards after each rest site's heal.

use sts2_core::{EventHook, HookEffect};

/// `TryModifyRestSiteHealRewards`: two potion rewards for its owner, an
/// event's heal included. A run here has one player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::RestSiteHealRewards { .. } = hook {
        asked.push(HookEffect::OfferRewards(V::TINY_MAILBOX_REWARDS));
    }
}

/// `REWARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `TryModifyRestSiteHealRewards`'s two `PotionReward(player)`.
        const TINY_MAILBOX_REWARDS: &'static [sts2_core::CustomReward<$build>] = &[
            sts2_core::CustomReward::Potion,
            sts2_core::CustomReward::Potion,
        ];
    };
}

pub(crate) use lists;
