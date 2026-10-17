//! `BoneFlute`: block each time Osty attacks.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::ids::MonsterIds as _;

/// `AfterAttack` by an Osty: its block on its owner, unpowered. A fight here
/// has one player, whose pet every Osty is.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::AfterAttack {
        monster: Some(__c0),
        ..
    } = hook
        && __c0 == V::Monster::OSTY
    {
        asked.push(HookEffect::GainBlock(V::Relic::BONE_FLUTE_BLOCK));
    }
}
