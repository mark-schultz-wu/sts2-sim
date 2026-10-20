//! `BoomingConch`: two more cards and an energy to open an elite fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::generated::encounters::RoomType;
use crate::generated::runstate::CombatSide;

/// `ModifyHandDraw`: two more cards on an elite fight's first turn.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HandDraw {
            turn,
            room: RoomType::Elite,
            ..
        } if turn <= 1 => Modify::Added(V::Relic::BOOMING_CONCH_CARDS),
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what BoomingConch does for {hook:?}"),
    }
}

/// `AfterSideTurnStart`: an energy on an elite fight's first player turn.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn,
        room: RoomType::Elite,
        ..
    } = hook
        && turn <= 1
    {
        asked.push(HookEffect::GainEnergy(V::Relic::BOOMING_CONCH_ENERGY));
    }
}
