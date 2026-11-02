//! `BagOfMarbles`: Vulnerable on every enemy as a fight opens.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::ids::PowerIds as _;

/// `BeforeSideTurnStart`: Vulnerable on every hittable enemy as its owner's
/// first turn begins.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::BeforeSideTurnStart {
        participants, turn, ..
    } = hook
        && participants.contains(owner)
        && turn <= 1
    {
        asked.push(HookEffect::ApplyToOpponents(
            V::Power::VULNERABLE_POWER,
            PowerAmount::Counted(V::Relic::BAG_OF_MARBLES_VULNERABLE_POWER),
        ));
    }
}
