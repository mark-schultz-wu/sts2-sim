//! `AmethystAubergine`: more gold from every fight but the last boss.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::encounters::RoomType;

/// `GoldVar(15)`.
const GOLD: Number = 15;

/// `TryModifyRewards`: a `GoldReward` of its gold in a fight's rewards, unless
/// the fight is the last act's boss.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::ModifyRewards {
        room: Some(room),
        act,
        acts,
    } = hook
        && crate::behaviour::relics::combat_room(room)
        && !(room == RoomType::Boss && act + 1 >= acts)
    {
        asked.push(HookEffect::AddGoldReward(GOLD));
    }
}
