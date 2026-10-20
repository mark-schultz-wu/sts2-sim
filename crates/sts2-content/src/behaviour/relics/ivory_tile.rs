//! `IvoryTile`: energy back for a card that spent three or more.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `EnergyVar("EnergyThreshold", 3)`.
const THRESHOLD: u32 = 3;

/// `AfterCardPlayed`: `PlayerCmd.GainEnergy` for a play of its owner's
/// that spent the threshold or more. A fight here has one player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { spent, .. } = hook
        && spent >= THRESHOLD
    {
        asked.push(HookEffect::GainEnergy(ENERGY));
    }
}
