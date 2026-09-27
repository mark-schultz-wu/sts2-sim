//! `BoneFlute`: block each time Osty attacks.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::monsters::Monster;
use super::Number;

/// `BlockVar(2, ValueProp.Unpowered)`.
const BLOCK: Number = 2;

/// `AfterAttack` by an Osty: its block on its owner, unpowered. A fight here
/// has one player, whose pet every Osty is.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::AfterAttack {
        monster: Some(Monster::Osty),
        ..
    } = hook
    {
        asked.push(HookEffect::GainBlock(BLOCK));
    }
}
