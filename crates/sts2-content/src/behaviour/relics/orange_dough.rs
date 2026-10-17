//! `OrangeDough`: two colorless cards as each fight opens.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

/// `AfterSideTurnStart` on its owner's first turn: `GetDistinctForCombat`
/// of two colorless cards off `CombatCardGeneration`, into the hand. A fight
/// here has one player, who owns it.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn,
        ..
    } = hook
        && turn <= 1
    {
        asked.push(HookEffect::GenerateDistinctColorless {
            count: V::Relic::ORANGE_DOUGH_CARDS,
        });
    }
}
