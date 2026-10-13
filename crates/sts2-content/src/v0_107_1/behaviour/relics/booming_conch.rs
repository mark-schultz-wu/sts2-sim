//! `BoomingConch`: two more cards and an energy to open an elite fight.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `CardsVar(2)`.
const CARDS: Number = 2;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `ModifyHandDraw`: two more cards on an elite fight's first turn.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HandDraw {
            turn,
            room: RoomType::Elite,
            ..
        } if turn <= 1 => Modify::Added(CARDS),
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what BoomingConch does for {hook:?}"),
    }
}

/// `AfterSideTurnStart`: an energy on an elite fight's first player turn.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn,
        room: RoomType::Elite,
        ..
    } = hook
        && turn <= 1
    {
        asked.push(HookEffect::GainEnergy(ENERGY));
    }
}
