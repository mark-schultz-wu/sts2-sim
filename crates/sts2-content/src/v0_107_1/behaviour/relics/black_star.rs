//! `BlackStar`: a second relic from every elite.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;

/// `TryModifyRewards`: one more `RelicReward` in an elite's rewards. A
/// relic's owner is the player, and a run here has one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::ModifyRewards {
            room: Some(RoomType::Elite),
            ..
        } => asked.push(HookEffect::AddRelicRewards(1)),
        EventHook::ModifyRewards { .. } | EventHook::Obtained => {}
        hook => unimplemented!("what BlackStar does for {hook:?}"),
    }
}
