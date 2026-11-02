//! `BoneFlute`: block each time Osty attacks.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::ids::MonsterIds as _;

/// `AfterAttack` by its owner's Osty: its block on its owner, unpowered.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::AfterAttack {
        monster: Some(__c0),
        pet_of,
        ..
    } = hook
        && __c0 == V::Monster::OSTY
        && pet_of == Some(owner)
    {
        asked.push(HookEffect::GainBlock(V::Relic::BONE_FLUTE_BLOCK));
    }
}
