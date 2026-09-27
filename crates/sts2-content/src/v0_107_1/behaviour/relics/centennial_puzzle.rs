//! `CentennialPuzzle`: cards drawn the first time its owner loses hit points
//! in a fight.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `CardsVar(3)`.
const CARDS: Number = 3;

/// `UsedThisCombat`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    used_this_combat: bool,
}

/// `AfterDamageReceived` on its owner, for a hit that got past the block: its
/// cards, once a fight. `AfterCombatEnd` makes it ready again.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::DamageReceived {
            target,
            unblocked,
            player,
            ..
        } if target == player && unblocked > 0 && !state.used_this_combat => {
            state.used_this_combat = true;
            // One `CardPileCmd.Draw` a card, which draws the same as one for
            // all three.
            asked.push(HookEffect::Draw(CARDS));
        }
        EventHook::CombatEnd { .. } => state.used_this_combat = false,
        _ => {}
    }
}
