//! `RippleBasin`: block at the end of a turn with no attack played.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect};

/// Whether one of its owner's attacks finished a play this turn, which
/// `BeforeSideTurnEnd` reads off `CardPlaysFinished`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    attacked: bool,
}

/// `BeforeSideTurnEnd` on its owner's side: its block, unpowered, where no
/// attack of its owner's finished a play this turn.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlayed { card, player, .. }
            if player == owner
                && <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Attack) =>
        {
            state.attacked = true;
        }
        EventHook::BeforeSideTurnEnd { participants, .. }
            if participants.contains(owner) && !state.attacked =>
        {
            asked.push(HookEffect::GainBlock(V::Relic::RIPPLE_BASIN_BLOCK))
        }
        EventHook::BeforeSideTurnStart { participants, .. } if participants.contains(owner) => {
            state.attacked = false
        }
        EventHook::CombatEnd { .. } => state.attacked = false,
        _ => {}
    }
}
