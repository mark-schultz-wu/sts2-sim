//! `BigHat`: two Ethereal cards into hand as the first turn of a fight
//! starts.

use sts2_core::{EventHook, HookEffect, PoolFilter};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `CardsVar(2)`.
const CARDS: Number = 2;

/// `AfterSideTurnStart`, for its owner's first turn: `GetDistinctForCombat`
/// of two of the character's Ethereal cards off `CombatCardGeneration`, into
/// hand. A relic's owner is the player, and a fight here has one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn: 1,
        ..
    } = hook
    {
        asked.push(HookEffect::GenerateDistinctFromPool {
            filter: PoolFilter::Ethereal,
            count: CARDS,
        });
    }
}
