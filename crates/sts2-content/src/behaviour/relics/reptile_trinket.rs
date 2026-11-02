//! `ReptileTrinket`: Strength for the turn after each potion drunk in a fight.

use crate::ids::PowerIds as _;
use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

/// `AfterPotionUsed`: its Strength as Reptile Trinket on its owner, for a
/// potion of its owner's drunk while a fight is in progress.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::PotionUsed {
        player,
        in_fight: true,
        ..
    } = hook
        && player == owner
    {
        asked.push(HookEffect::ApplyToOwner(
            V::Power::REPTILE_TRINKET_POWER,
            PowerAmount::Counted(V::Relic::REPTILE_TRINKET_STRENGTH_POWER),
        ));
    }
}
