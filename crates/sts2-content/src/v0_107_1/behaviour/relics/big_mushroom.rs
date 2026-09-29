//! `BigMushroom`: more maximum health, and a smaller first hand.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `MaxHpVar(20)`.
const MAX_HP: Number = 20;

/// `CardsVar(2)`.
const CARDS: Number = 2;

/// `AfterObtained`: `CreatureCmd.GainMaxHp` on its owner. Its
/// `AfterRoomEntered` only scales its owner's sprite.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainMaxHp(MAX_HP));
    }
}

/// `ModifyHandDraw`: two fewer cards on its owner's first turn. A fight here
/// has one player.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HandDraw { turn: 1, .. } => Modify::Added(-CARDS),
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what BigMushroom does for {hook:?}"),
    }
}
