//! `DemonTongue`: the first hit to get past its owner's block on its own
//! turn is healed back.

use sts2_core::{EventHook, HookEffect, SideKinds as _};

/// `_triggeredThisTurn`, which no fight's end resets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    triggered_this_turn: bool,
}

/// `AfterDamageReceived` on its owner in the owner's side's turn, for a hit
/// that got past the block: `CreatureCmd.Heal` by what got through, once a
/// turn. `BeforeSideTurnStart` of the owner's side makes it ready again.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let own = <V as sts2_core::ContentVersion>::CombatSide::PLAYER;
    match hook {
        EventHook::DamageReceived {
            target,
            unblocked,
            player,
            side,
            ..
        } if side == own && target == player && unblocked > 0 && !state.triggered_this_turn => {
            state.triggered_this_turn = true;
            asked.push(HookEffect::Heal(unblocked));
        }
        EventHook::BeforeSideTurnStart { side, .. } if side == own => {
            state.triggered_this_turn = false;
        }
        EventHook::DamageReceived { .. }
        | EventHook::BeforeSideTurnStart { .. }
        | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what DemonTongue does for {hook:?}"),
    }
}
