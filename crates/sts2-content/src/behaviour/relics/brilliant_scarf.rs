//! `BrilliantScarf`: the fifth card its owner plays each turn is free.

use sts2_core::{EventHook, Modify, ModifyHook, Pile};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `CardsVar(5)`: the play that is free.
const CARDS: u8 = 5;

/// `CardsPlayedThisTurn`, the plays its owner made by hand.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    played: u8,
}

/// `AfterCardPlayed` counts a play no auto-play made. `BeforeSideTurnStart`
/// for its owner's side and `AfterCombatEnd` set the count back to none. A
/// relic's owner is the player, and a fight here has one, so every card is
/// its owner's.
pub(super) fn event_hook<V: crate::Build>(state: &mut State, hook: EventHook<V>) {
    match hook {
        EventHook::CardPlayed { auto: false, .. } => {
            state.played = state.played.saturating_add(1);
        }
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        }
        | EventHook::CombatEnd { .. } => state.played = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the plays made by hand this turn.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.played)
}

/// `TryModifyEnergyCostInCombatLate` and `TryModifyStarCost`: nothing, for a
/// card in hand or being played once four plays are made this turn.
pub(super) fn modify_hook<V: crate::Build>(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    let free =
        |pile: Option<Pile>| state.played == CARDS - 1 && matches!(pile, None | Some(Pile::Hand));
    match hook {
        ModifyHook::EnergyCostLate { pile, .. } | ModifyHook::StarCost { pile, .. }
            if free(pile) =>
        {
            Modify::Replaced(0)
        }
        ModifyHook::EnergyCostLate { .. } | ModifyHook::StarCost { .. } => Modify::Nothing,
        hook => unimplemented!("what BrilliantScarf does for {hook:?}"),
    }
}
