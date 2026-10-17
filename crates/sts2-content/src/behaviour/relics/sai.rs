//! `Sai`: block as each of its owner's turns begins.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

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
        asked.push(HookEffect::GainBlock(V::Relic::SAI_BLOCK));
    }
}
