//! `Regalite`: block for every card its owner makes in a fight.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `BlockVar(2, Unpowered)`.
const BLOCK: Number = 2;

/// `AfterCardGeneratedForCombat`: its block for each card its owner made. A
/// fight here has one player, the one creator a card can name.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::CardGeneratedForCombat {
        creator: Some(_), ..
    } = hook
    {
        asked.push(HookEffect::GainBlock(BLOCK));
    }
}
