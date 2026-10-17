//! `RingOfTheDrake`: Ring of the Snake refined, more cards for three turns.

use crate::vars::RelicVars as _;
use sts2_core::{Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyHandDraw`: two more cards up to the third turn, and none after.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        // A relic's owner is the player, and a fight here has one, so the
        // player drawing is always its own.
        ModifyHook::HandDraw { turn, .. } if turn <= (V::Relic::RING_OF_THE_DRAKE_TURNS as u32) => {
            Modify::Added(V::Relic::RING_OF_THE_DRAKE_CARDS)
        }
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what RingOfTheDrake does for {hook:?}"),
    }
}
