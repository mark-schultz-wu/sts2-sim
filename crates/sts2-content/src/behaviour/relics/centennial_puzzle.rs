//! `CentennialPuzzle`: cards drawn the first time its owner loses hit points
//! in a fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `UsedThisCombat`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    used_this_combat: bool,
}

/// `AfterDamageReceived` on its owner in a fight in progress, for a hit that
/// got past the block: its cards, once a fight. `AfterCombatEnd` makes it
/// ready again.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::DamageReceived {
            target,
            unblocked,
            player,
            in_progress: true,
            ..
        } if target == player && unblocked > 0 && !state.used_this_combat => {
            state.used_this_combat = true;
            // One `CardPileCmd.Draw` a card, which draws the same as one for
            // all three.
            asked.push(HookEffect::Draw(V::Relic::CENTENNIAL_PUZZLE_CARDS));
        }
        EventHook::CombatEnd { .. } => state.used_this_combat = false,
        _ => {}
    }
}
