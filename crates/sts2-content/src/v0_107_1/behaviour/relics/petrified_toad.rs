//! `PetrifiedToad`: a Potion-Shaped Rock as every fight starts.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::potions::Potion;

/// `BeforeCombatStartLate`: `PotionCmd.TryToProcure<PotionShapedRock>`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::CombatStartingLate { .. } = hook {
        asked.push(HookEffect::ProcurePotion(Potion::PotionShapedRock));
    }
}
