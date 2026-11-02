//! `BagOfPreparation`: more cards in the first hand of each fight.

use crate::vars::RelicVars as _;
use sts2_core::{Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyHandDraw`: two more cards on its owner's first turn. A fight here
/// has one player.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::HandDraw { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::HandDraw { turn, .. } if turn <= 1 => {
            Modify::Added(V::Relic::BAG_OF_PREPARATION_CARDS)
        }
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what BagOfPreparation does for {hook:?}"),
    }
}
