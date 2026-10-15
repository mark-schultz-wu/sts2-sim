//! `BeatingRemnant`: no more than twenty hit points lost in a turn.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `DynamicVar("MaxHpLoss", 20)`.
const MAX_HP_LOSS: Number = 20;

/// `DamageReceivedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    damage_received_this_turn: Number,
}

/// `AfterDamageReceived` on its owner, in a fight in progress, adds what the
/// hit took.
/// `BeforeSideTurnStart` on its owner's side starts the count again.
pub(super) fn event_hook(state: &mut State, hook: EventHook<V>) {
    match hook {
        EventHook::DamageReceived {
            target,
            unblocked,
            player,
            in_progress: true,
            ..
        } if target == player => state.damage_received_this_turn += unblocked,
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        } => state.damage_received_this_turn = 0,
        _ => {}
    }
}

/// `ModifyHpLostAfterOsty` on its owner in a fight: no more than what the
/// turn has left of its loss.
pub(super) fn modify_hook(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HpLostAfterOsty {
            target,
            player,
            in_fight: true,
        } if target == player => Modify::AtMost(MAX_HP_LOSS - state.damage_received_this_turn),
        ModifyHook::HpLostAfterOsty { .. } => Modify::Nothing,
        hook => unimplemented!("what BeatingRemnant does for {hook:?}"),
    }
}
