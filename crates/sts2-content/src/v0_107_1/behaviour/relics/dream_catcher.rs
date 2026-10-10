//! `DreamCatcher`: a card reward after each rest site's heal.

use sts2_core::{CustomReward, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `TryModifyRestSiteHealRewards`'s `CardReward(ForRoom(player, Monster), 3)`.
const REWARDS: [CustomReward<V>; 1] = [CustomReward::MonsterRoomCards];

/// `TryModifyRestSiteHealRewards`: a monster room's card reward for its
/// owner. `ModifyExtraRestSiteHealText` only adds a line on screen. A run
/// here has one player.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::RestSiteHealRewards { .. } => asked.push(HookEffect::OfferRewards(&REWARDS)),
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what DreamCatcher does for {hook:?}"),
    }
}
