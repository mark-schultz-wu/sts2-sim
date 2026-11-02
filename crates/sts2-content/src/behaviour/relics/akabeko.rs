//! `Akabeko`: Vigor to open each fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::ids::PowerIds as _;

/// `AfterSideTurnStart`: Vigor on its owner as its first turn begins.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        participants, turn, ..
    } = hook
        && participants.contains(owner)
        && turn <= 1
    {
        asked.push(HookEffect::ApplyToOwner(
            V::Power::VIGOR_POWER,
            PowerAmount::Counted(V::Relic::AKABEKO_VIGOR_POWER),
        ));
    }
}
