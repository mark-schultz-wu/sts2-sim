//! `TheAbacus`: block each time its owner shuffles.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `BlockVar(6, ValueProp.Unpowered)`.
const BLOCK: Number = 6;

/// `AfterShuffle` of its owner's draw pile: `CreatureCmd.GainBlock`. A
/// fight here has one player, who does every shuffle.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Shuffled => asked.push(HookEffect::GainBlock(BLOCK)),
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what TheAbacus does for {hook:?}"),
    }
}
