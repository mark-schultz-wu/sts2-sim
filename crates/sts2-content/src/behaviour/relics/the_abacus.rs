//! `TheAbacus`: block each time its owner shuffles.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterShuffle` of its owner's draw pile: `CreatureCmd.GainBlock`. A
/// fight here has one player, who does every shuffle.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Shuffled => asked.push(HookEffect::GainBlock(V::Relic::THE_ABACUS_BLOCK)),
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what TheAbacus does for {hook:?}"),
    }
}
