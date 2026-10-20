//! `Gremlin Horn`: a card and energy after an enemy dies.

use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

/// Its death response is delivered after an enemy dies and combat continues.
/// The other lifecycle notifications do nothing.
///
/// `target.Side != Owner.Creature.Side` is the whole of its rule. A relic's
/// owner is a player. So this pays for a creature that fell on the other side,
/// and not for the player holding it.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::AfterDeath { side, .. }
            if side != <CombatSide as sts2_core::SideKinds>::PLAYER =>
        {
            asked.extend([HookEffect::GainEnergy(1), HookEffect::Draw(1)])
        }
        _ => {}
    }
}
