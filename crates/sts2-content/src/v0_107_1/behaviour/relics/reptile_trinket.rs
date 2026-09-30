//! `ReptileTrinket`: Strength for the turn after each potion drunk in a fight.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;

/// `PowerVar<StrengthPower>(3)`.
const STRENGTH: PowerAmount<super::Number> = PowerAmount::Counted(3);

/// `AfterPotionUsed`: its Strength as Reptile Trinket on its owner, for a
/// potion drunk while a fight is in progress. A run here has one player,
/// whose potions are all its owner's.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::PotionUsed { in_fight: true, .. } = hook {
        asked.push(HookEffect::ApplyToOwner(
            Power::ReptileTrinketPower,
            STRENGTH,
        ));
    }
}
