//! `Akabeko`: Vigor to open each fight.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::super::super::generated::runstate::CombatSide;

/// `PowerVar<VigorPower>(8)`.
const VIGOR: PowerAmount<super::Number> = PowerAmount::Counted(8);

/// `AfterSideTurnStart`: Vigor on its owner as its first turn begins.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn,
        ..
    } = hook
        && turn <= 1
    {
        asked.push(HookEffect::ApplyToOwner(Power::VigorPower, VIGOR));
    }
}
