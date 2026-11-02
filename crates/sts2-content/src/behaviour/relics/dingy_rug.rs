//! `DingyRug`: colorless cards among a card reward's.

use sts2_core::{EventHook, HookEffect};

/// `ModifyCardRewardCreationOptions`: the colorless pool after the reward's
/// own, where it is not among them. The run asks only for a `CardReward`
/// whose options take a change to their pools.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardRewardCreationOptions { player, .. } if player == owner => {
            asked.push(HookEffect::AddColorlessPool)
        }
        EventHook::CardRewardCreationOptions { .. } => {}
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what DingyRug does for {hook:?}"),
    }
}
