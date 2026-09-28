//! `VexingPuzzlebox`: a free card of the character's pool on the first turn.

use sts2_core::{Change, CostPool, EventHook, HookEffect, Lasting, PoolFilter};

use super::super::super::V0_107_1 as V;

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

/// `AfterPlayerTurnStart` on its owner's first turn: one card of the
/// character's pool by `GetDistinctForCombat` off `CombatCardGeneration`,
/// free this turn, into hand.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::PlayerTurnStart { turn, .. } = hook
        && turn == 1
    {
        asked.push(HookEffect::GenerateFromPool {
            filter: PoolFilter::Every,
            count: 1,
            writes: FREE_THIS_TURN,
        });
    }
}
