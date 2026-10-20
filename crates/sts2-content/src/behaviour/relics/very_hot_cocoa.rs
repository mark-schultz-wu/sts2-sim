//! `VeryHotCocoa`: energy on the first turn of each fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

/// `AfterSideTurnStart`: `PlayerCmd.GainEnergy` while its owner's
/// `TurnNumber` is at most one. A relic's owner is the player, and a fight
/// here has one, so the player's side holds its owner.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::SideTurnStart {
            side: CombatSide::Player,
            turn,
            ..
        } if turn <= 1 => asked.push(HookEffect::GainEnergy(V::Relic::VERY_HOT_COCOA_ENERGY)),
        _ => {}
    }
}
