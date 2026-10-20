//! `Akabeko`: Vigor to open each fight.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::generated::runstate::CombatSide;
use crate::ids::PowerIds as _;

/// `PowerVar<VigorPower>(8)`.
const VIGOR: PowerAmount<crate::behaviour::relics::Number> = PowerAmount::Counted(8);

/// `AfterSideTurnStart`: Vigor on its owner as its first turn begins.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn,
        ..
    } = hook
        && turn <= 1
    {
        asked.push(HookEffect::ApplyToOwner(V::Power::VIGOR_POWER, VIGOR));
    }
}
