//! `SymbioticVirus`: a Dark orb as each fight opens.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;
use crate::ids::OrbIds as _;

/// `AfterSideTurnStart` on its owner's first turn. A fight here has one
/// player, who owns it.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        turn,
        ..
    } = hook
        && turn <= 1
    {
        asked.extend(
            (0..(V::Relic::SYMBIOTIC_VIRUS_DARK as usize))
                .map(|_| HookEffect::Channel(V::Orb::DARK_ORB)),
        );
    }
}
