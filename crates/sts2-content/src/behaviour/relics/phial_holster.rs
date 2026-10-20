//! `PhialHolster`: a potion slot, and two potions to put in the belt.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::RunRngType;

/// `AfterObtained`: `PlayerCmd.GainMaxPotionCount`, then two potions from
/// `CreateRandomPotionsOutOfCombat` off `RunState.Rng.CombatPotionGeneration`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::GainPotionSlots(V::Relic::PHIAL_HOLSTER_POTION_SLOTS as usize),
            HookEffect::ProcureRandomPotions {
                in_combat: false,
                count: (V::Relic::PHIAL_HOLSTER_POTIONS as usize),
                stream: RunRngType::CombatPotionGeneration,
                into_gained: false,
            },
        ]);
    }
}
