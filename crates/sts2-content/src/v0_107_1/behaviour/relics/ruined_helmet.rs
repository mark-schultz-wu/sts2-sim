//! `RuinedHelmet`: the first Strength its owner takes in a fight, doubled.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::Number;

/// `UsedThisCombat`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    used_this_combat: bool,
}

/// `TryModifyPowerAmountReceived`: Strength landing on its owner doubled,
/// once a fight.
pub(super) fn modify_hook(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::PowerAmountReceived {
            power: Power::StrengthPower,
            target,
            amount,
            player,
        } if target == player && amount > 0 && !state.used_this_combat => {
            Modify::Replaced(amount * 2)
        }
        _ => Modify::Nothing,
    }
}

/// `AfterModifyingPowerAmountReceived` spends it for the fight, and
/// `AfterCombatEnd` gives it back.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::PowerAmountReceivedModified { .. } => state.used_this_combat = true,
        EventHook::CombatEnd { .. } => state.used_this_combat = false,
        EventHook::Obtained => {}
        hook => unimplemented!("what RuinedHelmet does for {hook:?}"),
    }
}
