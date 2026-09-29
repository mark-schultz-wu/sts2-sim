//! `PhialHolster`: a potion slot, and two potions to put in the belt.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::RunRngType;

/// `DynamicVar("PotionSlots", 1)`.
const SLOTS: usize = 1;

/// `DynamicVar("Potions", 2)`.
const POTIONS: usize = 2;

/// `AfterObtained`: `PlayerCmd.GainMaxPotionCount`, then two potions from
/// `CreateRandomPotionsOutOfCombat` off `RunState.Rng.CombatPotionGeneration`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::GainPotionSlots(SLOTS),
            HookEffect::ProcureRandomPotions {
                in_combat: false,
                count: POTIONS,
                stream: RunRngType::CombatPotionGeneration,
                into_gained: false,
            },
        ]);
    }
}
