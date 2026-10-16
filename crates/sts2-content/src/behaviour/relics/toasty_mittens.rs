//! `ToastyMittens`: the top card of the draw pile burned, and a point of
//! Strength, before every hand.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::behaviour::relics::Number;
use crate::ids::PowerIds as _;

/// `PowerVar<StrengthPower>(1)`.
const STRENGTH: Number = 1;

/// `BeforeHandDraw`: `ShuffleIfNecessary`, then `CardCmd.Exhaust` on the top
/// card of the draw pile. The first turn passes over the innate ones. Then
/// its Strength goes on its owner. A relic's owner is the player, and a
/// fight here has one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::BeforeHandDraw { turn, .. } => asked.extend([
            HookEffect::ShuffleIfNecessary,
            HookEffect::ExhaustFromDrawTop {
                passing_innate: turn == 1,
            },
            HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, PowerAmount::Counted(STRENGTH)),
        ]),
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what ToastyMittens does for {hook:?}"),
    }
}
