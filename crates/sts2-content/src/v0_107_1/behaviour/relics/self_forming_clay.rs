//! `SelfFormingClay`: block next turn after each hit that gets through.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;

/// `DynamicVar("BlockNextTurn", 3)`.
const BLOCK_NEXT_TURN: PowerAmount<super::Number> = PowerAmount::Counted(3);

/// `AfterDamageReceived` on its owner in a fight in progress, for a hit that
/// took hit points: its block as Self-Forming Clay.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
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
            Power::SelfFormingClayPower,
            BLOCK_NEXT_TURN,
        ));
    }
}
