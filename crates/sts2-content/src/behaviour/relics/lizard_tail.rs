//! `LizardTail`: once a run, half its owner's health back instead of dying.

use crate::vars::RelicVars as _;
use sts2_core::{DecisionHook, EventHook, HookEffect, HookResponse};

/// `WasUsed`, which it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    was_used: bool,
}

/// `IsUsedUp`, which is `WasUsed`.
pub(super) fn used_up(state: &State) -> bool {
    state.was_used
}

/// `ShouldDieLate`: its `owner` does not die while it is unused.
pub(super) fn decide_hook<V: crate::Build>(
    state: &State,
    owner: sts2_core::CombatId,
    hook: DecisionHook<V>,
) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldDieLate { creature } => {
            HookResponse::Decision(creature != owner || state.was_used)
        }
        _ => HookResponse::DoesNotImplement,
    }
}

/// `AfterPreventingDeath`: used, and `CreatureCmd.Heal` for half the maximum,
/// at least one. The health keeps the whole part.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::DeathPrevented { max_hp } => {
            state.was_used = true;
            asked.push(HookEffect::Heal(
                (max_hp * V::Relic::LIZARD_TAIL_HEAL / 100).max(1),
            ));
        }
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what LizardTail does for {hook:?}"),
    }
}
