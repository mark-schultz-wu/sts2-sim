//! `Driftwood`: every card reward can be rerolled once.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `TryModifyRewardsLate`: `CanReroll = true` on each card reward.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::ModifyRewardsLate => asked.push(HookEffect::RerollableCardRewards),
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what Driftwood does for {hook:?}"),
    }
}
