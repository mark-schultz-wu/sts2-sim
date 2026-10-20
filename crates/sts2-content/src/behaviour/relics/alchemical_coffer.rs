//! `AlchemicalCoffer`: four potion slots, and a potion in each.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::RunRngType;

/// `AfterObtained`: `PlayerCmd.GainMaxPotionCount`, then as many potions from
/// `CreateRandomPotionsOutOfCombat` off `RunState.Rng.CombatPotionGeneration`,
/// each into one of the slots just gained.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::GainPotionSlots(V::Relic::ALCHEMICAL_COFFER_POTION_SLOTS as usize),
            HookEffect::ProcureRandomPotions {
                in_combat: false,
                count: (V::Relic::ALCHEMICAL_COFFER_POTION_SLOTS as usize),
                stream: RunRngType::CombatPotionGeneration,
                into_gained: true,
            },
        ]);
    }
}
