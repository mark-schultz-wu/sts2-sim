//! `TwistedFunnel`: Poison on every enemy as a fight opens.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::super::super::generated::runstate::CombatSide;

/// `PowerVar<PoisonPower>(4)`.
const POISON: PowerAmount<super::Number> = PowerAmount::Counted(4);

/// `BeforeSideTurnStart`: Poison on every hittable enemy as its owner's
/// first turn begins.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::BeforeSideTurnStart {
        side: CombatSide::Player,
        turn,
    } = hook
        && turn <= 1
    {
        asked.push(HookEffect::ApplyToOpponents(Power::PoisonPower, POISON));
    }
}
