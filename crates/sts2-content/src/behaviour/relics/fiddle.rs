//! `Fiddle`: two more cards in each hand, and no draw on the player's own
//! turn besides it.

use crate::vars::RelicVars as _;
use sts2_core::{DecisionHook, HookResponse, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

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

/// `ModifyHandDrawLate`, or v0.111.0's `ModifyHandDraw`: two more, for its
/// owner. Each build's class overrides one of the two.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::HandDraw { player, .. } | ModifyHook::HandDrawLate { player, .. }
            if player != owner =>
        {
            Modify::Nothing
        }
        ModifyHook::HandDrawLate { .. } | ModifyHook::HandDraw { .. } => {
            Modify::Added(V::Relic::FIDDLE_CARDS)
        }
        hook => unimplemented!("what Fiddle does for {hook:?}"),
    }
}
