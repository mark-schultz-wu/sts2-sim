//! `AlchemicalCoffer`: four potion slots, and a potion in each.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::RunRngType;

/// `DynamicVar("PotionSlots", 4)`.
const SLOTS: usize = 4;

/// `AfterObtained`: `PlayerCmd.GainMaxPotionCount`, then as many potions from
/// `CreateRandomPotionsOutOfCombat` off `RunState.Rng.CombatPotionGeneration`,
/// each into one of the slots just gained.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained = hook {
        asked.extend([
            HookEffect::GainPotionSlots(SLOTS),
            HookEffect::ProcureRandomPotions {
                count: SLOTS,
                stream: RunRngType::CombatPotionGeneration,
                into_gained: true,
            },
        ]);
    }
}
