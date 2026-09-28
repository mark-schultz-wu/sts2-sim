//! `SwordOfStone`: the elites won, until five of them turn it to jade.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::super::super::generated::relics::Relic;
use super::Number;

/// `DynamicVar("Elites", 5)`.
const ELITES: Number = 5;

/// `ElitesDefeated`, the one `[SavedProperty]` this relic declares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    elites_defeated: Number,
}

/// `AfterCombatVictory`: an elite room counts, and the fifth is
/// `RelicCmd.Replace` with Sword of Jade.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatVictory {
        room: RoomType::Elite,
    } = hook
    {
        state.elites_defeated += 1;
        if state.elites_defeated >= ELITES {
            asked.push(HookEffect::ReplaceRelic {
                from: Relic::SwordOfStone,
                to: Relic::SwordOfJade,
            });
        }
    }
}

/// `DisplayAmount`, which is `ElitesDefeated`.
pub(super) fn display_amount(state: &State) -> Number {
    state.elites_defeated
}
