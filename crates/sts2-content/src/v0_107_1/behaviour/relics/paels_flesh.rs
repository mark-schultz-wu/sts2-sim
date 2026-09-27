//! `PaelsFlesh`: one more energy from the third turn of each fight.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// The `TurnNumber` from which it adds.
const FROM_TURN: u32 = 3;

/// Its owner's `TurnNumber` while a fight runs, which its counter shows. The
/// fight tells it each turn through `BeforeSideTurnStart`, which the game
/// uses to refresh that counter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    turn: Option<u32>,
}

/// A relic's owner is the player, and a fight here has one, so the player's
/// side holds its owner.
pub(super) fn event_hook(state: &mut State, hook: EventHook<V>) {
    match hook {
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            turn,
        } => state.turn = Some(turn),
        EventHook::CombatEnd { .. } => state.turn = None,
        _ => {}
    }
}

/// `ModifyMaxEnergy`: its energy while its owner's `TurnNumber` is three or
/// more. A relic's owner is the player, and a fight here has one.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { turn, .. } if turn >= FROM_TURN => Modify::Added(ENERGY),
        ModifyHook::MaxEnergy { .. } => Modify::Nothing,
        hook => unimplemented!("what PaelsFlesh does for {hook:?}"),
    }
}

/// `DisplayAmount`: `PlayerCombatState?.TurnNumber ?? 1`.
pub(super) fn display_amount(state: &State) -> Number {
    state.turn.map_or(1, |turn| {
        Number::try_from(turn).expect("a turn number fits the game's numbers")
    })
}
