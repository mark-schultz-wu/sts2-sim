//! `DingyRug`: colorless cards among a card reward's.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `ModifyCardRewardCreationOptions`: the colorless pool after the reward's
/// own, where it is not among them. The run asks only for a `CardReward`
/// whose options take a change to their pools.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CardRewardCreationOptions => asked.push(HookEffect::AddColorlessPool),
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what DingyRug does for {hook:?}"),
    }
}
