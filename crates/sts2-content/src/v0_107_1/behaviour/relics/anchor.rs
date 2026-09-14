//! `Anchor`: the block a fight opens with.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `BlockVar(10)`, which is all this relic is.
const BLOCK: Number = 10;

/// `BeforeCombatStart`, which is `CreatureCmd.GainBlock` on its owner.
pub(super) fn event_hook(hook: EventHook<V>) -> Vec<HookEffect<V>> {
    match hook {
        EventHook::CombatStarting => vec![HookEffect::GainBlock(BLOCK)],
        _ => Vec::new(),
    }
}
