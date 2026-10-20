//! `Regalite`: block for every card its owner makes in a fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterCardGeneratedForCombat`: its block for each card its owner made. A
/// fight here has one player, the one creator a card can name.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardGeneratedForCombat {
        creator: Some(_), ..
    } = hook
    {
        asked.push(HookEffect::GainBlock(V::Relic::REGALITE_BLOCK));
    }
}
