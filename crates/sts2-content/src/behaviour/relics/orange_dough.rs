//! `OrangeDough`: two colorless cards as each fight opens.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `CardsVar(2)`.
const CARDS: Number = 2;

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
        asked.push(HookEffect::GenerateDistinctColorless { count: CARDS });
    }
}
