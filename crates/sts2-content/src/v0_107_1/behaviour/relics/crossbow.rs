//! `Crossbow`: a free Attack as each of its owner's turns begins.

use sts2_core::{CardKind, Change, CostPool, EventHook, HookEffect, Lasting, PoolFilter};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;

/// `SetToFreeThisTurn`: both costs set to nothing, until the turn ends or
/// the card is played.
const FREE_THIS_TURN: &[(Change<V>, Lasting)] = &[
    (
        Change::SetCost(CostPool::Energy, 0),
        Lasting::ThisTurnOrUntilPlayed,
    ),
    (
        Change::SetCost(CostPool::Star, 0),
        Lasting::ThisTurnOrUntilPlayed,
    ),
];

/// `AfterSideTurnStart`, for the side its owner is among: one Attack of the
/// character's pool by `GetDistinctForCombat` off `CombatCardGeneration`,
/// free this turn, into hand.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        ..
    } = hook
    {
        asked.push(HookEffect::GenerateFromPool {
            filter: PoolFilter::OfKind(CardKind::Attack),
            count: 1,
            writes: FREE_THIS_TURN,
        });
    }
}
