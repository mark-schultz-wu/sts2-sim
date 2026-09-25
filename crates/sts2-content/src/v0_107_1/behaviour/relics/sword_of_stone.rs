//! `SwordOfStone`: the elites won, until five of them turn it to jade.

use sts2_core::EventHook;

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::Number;

/// `DynamicVar("Elites", 5)`.
const ELITES: Number = 5;

/// `ElitesDefeated`, the one `[SavedProperty]` this relic declares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    elites_defeated: Number,
}

/// `AfterCombatVictory`: an elite room counts.
///
/// # Panics
///
/// The fifth, where `RelicCmd.Replace` swaps it for Sword of Jade.
pub(super) fn event_hook(state: &mut State, hook: EventHook<V>) {
    if let EventHook::CombatVictory {
        room: RoomType::Elite,
    } = hook
    {
        state.elites_defeated += 1;
        if state.elites_defeated >= ELITES {
            unimplemented!("Sword of Stone turning to Sword of Jade");
        }
    }
}

/// `DisplayAmount`, which is `ElitesDefeated`.
pub(super) fn display_amount(state: &State) -> Number {
    state.elites_defeated
}
