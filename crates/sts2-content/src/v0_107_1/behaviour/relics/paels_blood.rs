//! `PaelsBlood`: one more card in every hand.

use sts2_core::{Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `CardsVar(1)`.
const CARDS: Number = 1;

/// `ModifyHandDraw`: one more card each turn. A relic's owner is the player,
/// and a fight here has one, so the player drawing is always its own.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HandDraw { .. } => Modify::Added(CARDS),
        hook => unimplemented!("what PaelsBlood does for {hook:?}"),
    }
}
