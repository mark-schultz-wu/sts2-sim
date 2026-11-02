//! `NinjaScroll`: three Shivs in the first hand of every fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `BeforeHandDraw` on its owner's first turn: `Shiv.CreateInHand` of its
/// Shivs, before the draw. A relic's owner is the player, and a fight here
/// has one.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::BeforeHandDraw { player, .. } = hook
        && player != owner
    {
        return;
    }
    match hook {
        EventHook::BeforeHandDraw { turn: 1, .. } => {
            asked.push(HookEffect::GenerateInHand {
                card: V::Card::SHIV,
                count: V::Relic::NINJA_SCROLL_SHIVS,
            });
        }
        EventHook::BeforeHandDraw { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what NinjaScroll does for {hook:?}"),
    }
}
