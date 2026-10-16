//! `Fiddle`: two more cards in each hand, and no draw on the player's own
//! turn besides it.

use sts2_core::{DecisionHook, HookResponse, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `CardsVar(2)`.
const CARDS: Number = 2;

/// `ShouldDraw`: no for a draw that is not the hand's own while the player's
/// side takes its turn. A fight here has one player.
pub(super) fn decide_hook<V: crate::Build>(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldDraw {
            hand_draw, side, ..
        } => HookResponse::Decision(hand_draw || side != CombatSide::Player),
        hook => unimplemented!("what Fiddle decides for {hook:?}"),
    }
}

/// `ModifyHandDrawLate`: two more, for its owner.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HandDrawLate { .. } => Modify::Added(CARDS),
        hook => unimplemented!("what Fiddle does for {hook:?}"),
    }
}
