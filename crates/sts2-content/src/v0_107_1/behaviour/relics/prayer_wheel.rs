//! `PrayerWheel`: a second card reward from every monster fight.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;

/// `TryModifyRewards`: a card reward at a monster room's odds, in a monster
/// room's rewards. A run here has one player.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::ModifyRewards {
        room: Some(RoomType::Monster),
        ..
    } = hook
    {
        asked.push(HookEffect::AddCardReward(RoomType::Monster));
    }
}
