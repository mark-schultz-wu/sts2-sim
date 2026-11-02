//! `StoneCalendar`: damage to every enemy as the seventh turn ends.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// Its owner's `PlayerCombatState.TurnNumber`, which it reads, as the
/// fight's turns start. Nothing outside a fight.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    turn: u8,
}

/// `BeforeSideTurnEnd` on its owner's side: its damage to every hittable
/// enemy on the seventh turn.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::SideTurnStart {
            participants, turn, ..
        } if participants.contains(owner) => state.turn = u8::try_from(turn).unwrap_or(u8::MAX),
        EventHook::BeforeSideTurnEnd { participants, .. }
            if participants.contains(owner)
                && state.turn == (V::Relic::STONE_CALENDAR_DAMAGE_TURN as u8) =>
        {
            asked.push(HookEffect::DamageOpponents(V::Relic::STONE_CALENDAR_DAMAGE))
        }
        EventHook::CombatEnd { .. } => state.turn = 0,
        _ => {}
    }
}

/// `DisplayAmount`: its owner's `TurnNumber` until the seventh, and nothing
/// outside a fight or after it, which a player's death leaves the run. It
/// reads the turn rather than its own state, so a melted calendar still
/// counts.
pub(super) fn display_amount<V: crate::Build>(turn: Option<u32>) -> Number {
    match turn {
        Some(turn) if turn < u32::from(V::Relic::STONE_CALENDAR_DAMAGE_TURN as u8) => {
            Number::try_from(turn).expect("a turn below seven fits a game number")
        }
        _ => -1,
    }
}
