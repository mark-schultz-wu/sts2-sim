//! `BagOfPreparation`: more cards in the first hand of each fight.

use sts2_core::{Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `CardsVar(2)`.
const CARDS: Number = 2;

/// `ModifyHandDraw`: two more cards on its owner's first turn. A fight here
/// has one player.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HandDraw { turn, .. } if turn <= 1 => Modify::Added(CARDS),
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what BagOfPreparation does for {hook:?}"),
    }
}
