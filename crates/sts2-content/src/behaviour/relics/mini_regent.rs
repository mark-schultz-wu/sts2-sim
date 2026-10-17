//! `MiniRegent`: Strength the first time its owner spends stars in a turn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PowerAmount, SideKinds as _};

use crate::ids::PowerIds as _;

/// `UsedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    used: bool,
}

/// `AfterStarsSpent` for its owner, not yet used this turn, applies its
/// Strength and marks it used. `BeforeSideTurnStart` for the player's side
/// and `AfterCombatEnd` clear the mark.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::StarsSpent { .. } if !state.used => {
            state.used = true;
            asked.push(HookEffect::ApplyToOwner(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(V::Relic::MINI_REGENT_STRENGTH_POWER),
            ));
        }
        EventHook::BeforeSideTurnStart { side, .. }
            if side == <V as sts2_core::ContentVersion>::CombatSide::PLAYER =>
        {
            state.used = false;
        }
        EventHook::CombatEnd { .. } => state.used = false,
        _ => {}
    }
}
