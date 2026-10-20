//! `SelfFormingClay`: block next turn after each hit that gets through.

use crate::ids::PowerIds as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

/// `DynamicVar("BlockNextTurn", 3)`.
const BLOCK_NEXT_TURN: PowerAmount<crate::behaviour::relics::Number> = PowerAmount::Counted(3);

/// `AfterDamageReceived` on its owner in a fight in progress, for a hit that
/// took hit points: its block as Self-Forming Clay.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::DamageReceived {
        target,
        unblocked,
        player,
        in_progress: true,
        ..
    } = hook
        && target == player
        && unblocked > 0
    {
        asked.push(HookEffect::ApplyToOwner(
            V::Power::SELF_FORMING_CLAY_POWER,
            BLOCK_NEXT_TURN,
        ));
    }
}
