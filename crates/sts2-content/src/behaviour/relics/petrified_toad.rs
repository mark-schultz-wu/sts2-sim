//! `PetrifiedToad`: a Potion-Shaped Rock as every fight starts.

use crate::ids::PotionIds as _;
use sts2_core::{EventHook, HookEffect};

/// `BeforeCombatStartLate`: `PotionCmd.TryToProcure<PotionShapedRock>`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatStartingLate { .. } = hook {
        asked.push(HookEffect::ProcurePotion(V::Potion::POTION_SHAPED_ROCK));
    }
}
