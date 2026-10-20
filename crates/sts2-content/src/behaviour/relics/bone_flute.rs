//! `BoneFlute`: block each time Osty attacks.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::ids::MonsterIds as _;

/// `BlockVar(2, ValueProp.Unpowered)`.
const BLOCK: Number = 2;

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
        asked.push(HookEffect::GainBlock(BLOCK));
    }
}
