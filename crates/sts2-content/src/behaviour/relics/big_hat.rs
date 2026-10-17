//! `BigHat`: two Ethereal cards into hand as the first turn of a fight
//! starts.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PoolFilter};

use crate::generated::runstate::CombatSide;

/// `AfterSideTurnStart`, for its owner's first turn: `GetDistinctForCombat`
/// of two of the character's Ethereal cards off `CombatCardGeneration`, into
/// hand. A relic's owner is the player, and a fight here has one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn: 1,
        ..
    } = hook
    {
        asked.push(HookEffect::GenerateDistinctFromPool {
            filter: PoolFilter::Ethereal,
            count: V::Relic::BIG_HAT_CARDS,
        });
    }
}
