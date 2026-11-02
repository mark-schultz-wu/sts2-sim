//! `VeryHotCocoa`: energy on the first turn of each fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterSideTurnStart`: `PlayerCmd.GainEnergy` where its owner takes part
/// and its owner's `TurnNumber` is at most one. A fight here has one turn
/// number, which is its owner's.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::SideTurnStart {
            participants, turn, ..
        } if participants.contains(owner) && turn <= 1 => {
            asked.push(HookEffect::GainEnergy(V::Relic::VERY_HOT_COCOA_ENERGY))
        }
        _ => {}
    }
}
