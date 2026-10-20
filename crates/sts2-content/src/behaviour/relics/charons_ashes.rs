//! `CharonsAshes`: damage to every enemy for each card exhausted.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterCardExhausted`: its damage to every hittable enemy. A fight here
/// has one player, whose cards are all the owner's.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardExhausted { .. } = hook {
        asked.push(HookEffect::DamageOpponents(V::Relic::CHARONS_ASHES_DAMAGE));
    }
}
