//! `CharonsAshes`: damage to every enemy for each card exhausted.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterCardExhausted` of its owner's card: its damage to every hittable
/// enemy.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardExhausted { player, .. } = hook
        && player == owner
    {
        asked.push(HookEffect::DamageOpponents(V::Relic::CHARONS_ASHES_DAMAGE));
    }
}
