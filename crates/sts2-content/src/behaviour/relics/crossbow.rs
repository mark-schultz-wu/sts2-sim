//! `Crossbow`: a free Attack as each of its owner's turns begins.

use sts2_core::{CardKind, EventHook, HookEffect, PoolFilter};

/// `AfterSideTurnStart`, for the side its owner is among: one Attack of the
/// character's pool by `GetDistinctForCombat` off `CombatCardGeneration`,
/// free this turn, into hand.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart { participants, .. } = hook
        && participants.contains(owner)
    {
        asked.push(HookEffect::GenerateFromPool {
            filter: PoolFilter::OfKind(CardKind::Attack),
            count: 1,
            writes: V::CROSSBOW_FREE_THIS_TURN,
        });
    }
}

/// `FREE_THIS_TURN`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `SetToFreeThisTurn`: both costs set to nothing, until the turn ends or
        /// the card is played.
        const CROSSBOW_FREE_THIS_TURN: &'static [(
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
