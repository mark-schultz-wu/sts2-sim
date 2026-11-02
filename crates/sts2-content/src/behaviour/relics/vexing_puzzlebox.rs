//! `VexingPuzzlebox`: a free card of the character's pool on the first turn.

use sts2_core::{EventHook, HookEffect, PoolFilter};

/// `AfterPlayerTurnStart` on its owner's first turn: one card of the
/// character's pool by `GetDistinctForCombat` off `CombatCardGeneration`,
/// free this turn, into hand.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::PlayerTurnStart { player, .. } = hook
        && player != owner
    {
        return;
    }
    if let EventHook::PlayerTurnStart { turn, .. } = hook
        && turn == 1
    {
        asked.push(HookEffect::GenerateFromPool {
            filter: PoolFilter::Every,
            count: 1,
            writes: V::VEXING_PUZZLEBOX_FREE_THIS_TURN,
        });
    }
}

/// `FREE_THIS_TURN`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `SetToFreeThisTurn`: both costs set to nothing, until the turn ends or
        /// the card is played.
        const VEXING_PUZZLEBOX_FREE_THIS_TURN: &'static [(
            sts2_core::Change<$build>,
            sts2_core::Lasting,
        )] = &[
            (
                sts2_core::Change::SetCost(sts2_core::CostPool::Energy, 0),
                sts2_core::Lasting::ThisTurnOrUntilPlayed,
            ),
            (
                sts2_core::Change::SetCost(sts2_core::CostPool::Star, 0),
                sts2_core::Lasting::ThisTurnOrUntilPlayed,
            ),
        ];
    };
}

pub(crate) use lists;
