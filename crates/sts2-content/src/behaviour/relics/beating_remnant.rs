//! `BeatingRemnant`: no more than twenty hit points lost in a turn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `DamageReceivedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    damage_received_this_turn: Number,
}

/// `AfterDamageReceived` on its owner, in a fight in progress, adds what the
/// hit took.
/// `BeforeSideTurnStart` on its owner's side starts the count again.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
) {
    match hook {
        EventHook::DamageReceived {
            target,
            unblocked,
            in_progress: true,
            ..
        } if target == owner => state.damage_received_this_turn += unblocked,
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        } => state.damage_received_this_turn = 0,
        _ => {}
    }
}

/// `ModifyHpLostAfterOsty` on its owner in a fight: no more than what the
/// turn has left of its loss.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &State,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        ModifyHook::HpLostAfterOsty {
            target,
            in_fight: true,
        } if target == owner => {
            Modify::AtMost(V::Relic::BEATING_REMNANT_MAX_HP_LOSS - state.damage_received_this_turn)
        }
        ModifyHook::HpLostAfterOsty { .. } => Modify::Nothing,
        hook => unimplemented!("what BeatingRemnant does for {hook:?}"),
    }
}
