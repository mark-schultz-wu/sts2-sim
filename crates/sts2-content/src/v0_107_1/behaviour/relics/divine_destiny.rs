//! `DivineDestiny`: Divine Right refined, more stars as each fight opens.

use sts2_core::{EventHook, HookEffect, SideKinds};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `StarsVar(6)`.
const STARS: Number = 6;

/// `AfterSideTurnStart` on its owner's first turn, which is
/// `PlayerCmd.GainStars`. A relic's owner is the player, and a fight here has
/// one, so the player's side holds its owner.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::SideTurnStart { side, turn, .. }
            if side == <CombatSide as SideKinds>::PLAYER && turn <= 1 =>
        {
            asked.push(HookEffect::GainStars(STARS));
        }
        _ => {}
    }
}
