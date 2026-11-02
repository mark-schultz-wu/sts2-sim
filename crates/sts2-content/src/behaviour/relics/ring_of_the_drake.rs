//! `RingOfTheDrake`: Ring of the Snake refined, more cards for three turns.

use crate::vars::RelicVars as _;
use sts2_core::{Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyHandDraw`: two more cards up to the third turn, and none after.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::HandDraw { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::HandDraw { turn, .. } if turn <= (V::Relic::RING_OF_THE_DRAKE_TURNS as u32) => {
            Modify::Added(V::Relic::RING_OF_THE_DRAKE_CARDS)
        }
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what RingOfTheDrake does for {hook:?}"),
    }
}
