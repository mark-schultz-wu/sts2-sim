//! `RuinedHelmet`: the first Strength its owner takes in a fight, doubled.

use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::ids::PowerIds as _;

/// `UsedThisCombat`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    used_this_combat: bool,
}

/// `TryModifyPowerAmountReceived`: Strength landing on its owner doubled,
/// once a fight.
pub(super) fn modify_hook<V: crate::Build>(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::PowerAmountReceived {
            power: __c0,
            target,
            amount,
            player,
        } if __c0 == V::Power::STRENGTH_POWER
            && (target == player && amount > 0 && !state.used_this_combat) =>
        {
            Modify::Replaced(amount * 2)
        }
        _ => Modify::Nothing,
    }
}

/// `AfterModifyingPowerAmountReceived` spends it for the fight, and
/// `AfterCombatEnd` gives it back.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::PowerAmountReceivedModified { .. } => state.used_this_combat = true,
        EventHook::CombatEnd { .. } => state.used_this_combat = false,
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what RuinedHelmet does for {hook:?}"),
    }
}
