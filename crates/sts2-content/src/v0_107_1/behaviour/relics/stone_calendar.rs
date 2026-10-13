//! `StoneCalendar`: damage to every enemy as the seventh turn ends.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `DamageVar(52, Unpowered)`.
const DAMAGE: Number = 52;

/// `DynamicVar("DamageTurn", 7)`.
const DAMAGE_TURN: u8 = 7;

/// Its owner's `PlayerCombatState.TurnNumber`, which it reads, as the
/// fight's turns start. Nothing outside a fight.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    turn: u8,
}

/// `BeforeSideTurnEnd` on its owner's side: its damage to every hittable
/// enemy on the seventh turn.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::SideTurnStart {
            side: CombatSide::Player,
            turn,
            ..
        } => state.turn = u8::try_from(turn).unwrap_or(u8::MAX),
        EventHook::BeforeSideTurnEnd {
            side: CombatSide::Player,
            ..
        } if state.turn == DAMAGE_TURN => asked.push(HookEffect::DamageOpponents(DAMAGE)),
        EventHook::CombatEnd { .. } => state.turn = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the turn it stands on until the seventh, and nothing
/// outside a fight or after it, which a player's death leaves the run.
pub(super) fn display_amount(state: &State, fighting: bool) -> Number {
    if !fighting {
        return -1;
    }
    match state.turn {
        0 => -1,
        turn if turn >= DAMAGE_TURN => -1,
        turn => Number::from(turn),
    }
}
