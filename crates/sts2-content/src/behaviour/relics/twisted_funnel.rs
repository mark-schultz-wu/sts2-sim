//! `TwistedFunnel`: Poison on every enemy as a fight opens.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::generated::runstate::CombatSide;
use crate::ids::PowerIds as _;

/// `PowerVar<PoisonPower>(4)`.
const POISON: PowerAmount<crate::behaviour::relics::Number> = PowerAmount::Counted(4);

/// `BeforeSideTurnStart`: Poison on every hittable enemy as its owner's
/// first turn begins.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::BeforeSideTurnStart {
        side: CombatSide::Player,
        turn,
        ..
    } = hook
        && turn <= 1
    {
        asked.push(HookEffect::ApplyToOpponents(V::Power::POISON_POWER, POISON));
    }
}
