//! `FencingManual`: a Forge as a fight's first turn starts.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;
use crate::ids::CardIds as _;

/// `AfterSideTurnStart` on its owner's side, while the player's turn number
/// is one or less: `ForgeCmd.Forge` of its Forge.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::SideTurnStart {
            side: CombatSide::Player,
            turn,
            ..
        } if turn <= 1 => asked.push(HookEffect::Forge {
            card: V::Card::SOVEREIGN_BLADE,
            amount: V::Relic::FENCING_MANUAL_FORGE,
        }),
        EventHook::SideTurnStart { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what FencingManual does for {hook:?}"),
    }
}
