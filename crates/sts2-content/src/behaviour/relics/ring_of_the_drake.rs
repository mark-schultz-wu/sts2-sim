//! `RingOfTheDrake`: Ring of the Snake refined, more cards for three turns.

use sts2_core::{Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `CardsVar(2)`.
const CARDS: Number = 2;

/// `DynamicVar("Turns", 3)`: the last turn that draws more.
const TURNS: u32 = 3;

/// `ModifyHandDraw`: two more cards up to the third turn, and none after.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        // A relic's owner is the player, and a fight here has one, so the
        // player drawing is always its own.
        ModifyHook::HandDraw { turn, .. } if turn <= TURNS => Modify::Added(CARDS),
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what RingOfTheDrake does for {hook:?}"),
    }
}
