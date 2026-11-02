//! `DivineDestiny`: Divine Right refined, more stars as each fight opens.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterSideTurnStart` on its owner's first turn, which is
/// `PlayerCmd.GainStars`. A relic's owner is the player, and a fight here has
/// one, so the player's side holds its owner.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::SideTurnStart {
            participants, turn, ..
        } if participants.contains(owner) && turn <= 1 => {
            asked.push(HookEffect::GainStars(V::Relic::DIVINE_DESTINY_STARS));
        }
        _ => {}
    }
}
