//! `BigMushroom`: more maximum health, and a smaller first hand.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `AfterObtained`: `CreatureCmd.GainMaxHp` on its owner. Its
/// `AfterRoomEntered` only scales its owner's sprite.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainMaxHp(V::Relic::BIG_MUSHROOM_MAX_HP));
    }
}

/// `ModifyHandDraw`: two fewer cards on its owner's first turn. A fight here
/// has one player.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HandDraw { turn: 1, .. } => Modify::Added(-V::Relic::BIG_MUSHROOM_CARDS),
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what BigMushroom does for {hook:?}"),
    }
}
