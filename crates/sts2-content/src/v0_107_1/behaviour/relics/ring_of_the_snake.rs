//! `RingOfTheSnake`: the Silent's extra draw on a fight's first turn.

use sts2_core::{Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `CardsVar(2)`.
const CARDS: Number = 2;

/// `ModifyHandDraw`: two more cards on the first turn, and none after.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        // A relic's owner is the player, and a fight here has one, so the
        // player drawing is always its own.
        ModifyHook::HandDraw { turn, .. } if turn <= 1 => Modify::Added(CARDS),
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what RingOfTheSnake does for {hook:?}"),
    }
}
