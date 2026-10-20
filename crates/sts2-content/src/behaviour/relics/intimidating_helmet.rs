//! `IntimidatingHelmet`: block before each play that spends two energy or
//! more.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `BlockVar(4, ValueProp.Unpowered)`.
const BLOCK: Number = 4;

/// `EnergyVar(2)`: the least a play spends to set it off.
const ENERGY: Number = 2;

/// `BeforeCardPlayed`: `CreatureCmd.GainBlock` on its owner when the play's
/// `Resources.EnergyValue` reaches its energy. A relic's owner is the player,
/// and a fight here has one, so every card played is its owner's.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlaying { spent, .. } if spent >= ENERGY => {
            asked.push(HookEffect::GainBlock(BLOCK));
        }
        EventHook::CardPlaying { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what IntimidatingHelmet does for {hook:?}"),
    }
}
