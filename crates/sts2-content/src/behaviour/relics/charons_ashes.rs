//! `CharonsAshes`: damage to every enemy for each card exhausted.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `DamageVar(3, Unpowered)`.
const DAMAGE: Number = 3;

/// `AfterCardExhausted`: its damage to every hittable enemy. A fight here
/// has one player, whose cards are all the owner's.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardExhausted { .. } = hook {
        asked.push(HookEffect::DamageOpponents(DAMAGE));
    }
}
