//! `WhiteStar`: a card reward at a boss's odds from every elite.

use sts2_core::{EventHook, HookEffect};

use crate::generated::encounters::RoomType;

/// `TryModifyRewards`: a card reward at a boss room's odds, in an elite's
/// rewards. A run here has one player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::ModifyRewards {
        room: Some(RoomType::Elite),
        ..
    } = hook
    {
        asked.push(HookEffect::AddCardReward(RoomType::Boss));
    }
}
