//! `Sai`: block as each of its owner's turns begins.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `BlockVar(7, Unpowered)`.
const BLOCK: Number = 7;

/// `AfterSideTurnStart`: its block on its owner, for the side its owner is
/// among.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        ..
    } = hook
    {
        asked.push(HookEffect::GainBlock(BLOCK));
    }
}
