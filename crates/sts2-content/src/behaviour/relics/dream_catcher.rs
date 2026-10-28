//! `DreamCatcher`: a card reward after each rest site's heal.

use sts2_core::{EventHook, HookEffect};

/// `TryModifyRestSiteHealRewards`: a monster room's card reward for its
/// owner. `ModifyExtraRestSiteHealText` only adds a line on screen.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::RestSiteHealRewards { player, .. } if player == owner => {
            asked.push(HookEffect::OfferRewards(V::DREAM_CATCHER_REWARDS))
        }
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what DreamCatcher does for {hook:?}"),
    }
}

/// `REWARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `TryModifyRestSiteHealRewards`'s `CardReward(ForRoom(player, Monster), 3)`.
        const DREAM_CATCHER_REWARDS: &'static [sts2_core::CustomReward<$build>] =
            &[sts2_core::CustomReward::MonsterRoomCards];
    };
}

pub(crate) use lists;
