//! `AmethystAubergine`: more gold from every fight but the last boss.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::encounters::RoomType;

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
        asked.push(HookEffect::AddGoldReward(V::Relic::AMETHYST_AUBERGINE_GOLD));
    }
}
