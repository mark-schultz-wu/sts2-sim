//! `LizardTail`: once a run, half its owner's health back instead of dying.

use sts2_core::{DecisionHook, EventHook, HookEffect, HookResponse};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `HealVar(50)`, a share of the maximum in hundredths.
const HEAL: Number = 50;

/// `WasUsed`, which it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    was_used: bool,
}

/// `ShouldDieLate`: its owner does not die while it is unused.
pub(super) fn decide_hook(state: &State, hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldDieLate { creature, player } => {
            HookResponse::Decision(creature != player || state.was_used)
        }
        _ => HookResponse::DoesNotImplement,
    }
}

/// `AfterPreventingDeath`: used, and `CreatureCmd.Heal` for half the maximum,
/// at least one. The health keeps the whole part.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::DeathPrevented { max_hp } => {
            state.was_used = true;
            asked.push(HookEffect::Heal((max_hp * HEAL / 100).max(1)));
        }
        EventHook::Obtained => {}
        hook => unimplemented!("what LizardTail does for {hook:?}"),
    }
}
