//! `ToughBandages`: block for each card its owner discards in its own turn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

/// `AfterCardDiscarded` of its owner's card in its owner's side's turn: its
/// block on its owner.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardDiscarded {
            player,
            side: CombatSide::Player,
        } if player == owner => asked.push(HookEffect::GainBlock(V::Relic::TOUGH_BANDAGES_BLOCK)),
        EventHook::CardDiscarded { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what ToughBandages does for {hook:?}"),
    }
}
