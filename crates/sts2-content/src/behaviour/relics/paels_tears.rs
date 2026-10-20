//! `PaelsTears`: energy on a turn after one that ended with some left.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

/// `HadLeftoverEnergy`, which it keeps between one turn's end and the next
/// turn's start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    had_leftover: bool,
}

/// A relic's owner is the player, and a fight here has one, so the player's
/// side holds its owner.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        // `BeforeSideTurnEnd`: whether the player's energy is above nothing.
        EventHook::BeforeSideTurnEnd {
            side: CombatSide::Player,
            energy,
            ..
        } => state.had_leftover = energy > 0,
        // `AfterSideTurnStart`: `PlayerCmd.GainEnergy` after a turn that
        // ended with energy left.
        EventHook::SideTurnStart {
            side: CombatSide::Player,
            ..
        } if state.had_leftover => asked.push(HookEffect::GainEnergy(V::Relic::PAELS_TEARS_ENERGY)),
        EventHook::CombatEnd { .. } => state.had_leftover = false,
        _ => {}
    }
}
