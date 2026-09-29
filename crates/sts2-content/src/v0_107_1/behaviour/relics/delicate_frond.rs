//! `DelicateFrond`: every open potion slot filled as a fight opens.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::RunRngType;

/// `BeforeCombatStart`: while the belt has an open slot, a random potion off
/// `CombatPotionGeneration`, procured.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CombatStarting { .. } => asked.push(HookEffect::FillPotionSlots {
            stream: RunRngType::CombatPotionGeneration,
        }),
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what DelicateFrond does for {hook:?}"),
    }
}
