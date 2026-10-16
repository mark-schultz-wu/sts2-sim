//! `RippleBasin`: block at the end of a turn with no attack played.

use sts2_core::{CardKind, EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `BlockVar(4, ValueProp.Unpowered)`.
const BLOCK: Number = 4;

/// Whether one of its owner's attacks finished a play this turn, which
/// `BeforeSideTurnEnd` reads off `CardPlaysFinished`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    attacked: bool,
}

/// `BeforeSideTurnEnd` on its owner's side: its block, unpowered, where no
/// attack of its owner's finished a play this turn. A fight here has one
/// player, whose plays are all its owner's.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlayed { card, .. }
            if <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Attack) =>
        {
            state.attacked = true;
        }
        EventHook::BeforeSideTurnEnd {
            side: CombatSide::Player,
            ..
        } if !state.attacked => asked.push(HookEffect::GainBlock(BLOCK)),
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        }
        | EventHook::CombatEnd { .. } => state.attacked = false,
        _ => {}
    }
}
