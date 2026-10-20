//! `SwordOfStone`: the elites won, until five of them turn it to jade.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::encounters::RoomType;
use crate::ids::RelicIds as _;

/// `ElitesDefeated`, the one `[SavedProperty]` this relic declares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    elites_defeated: Number,
}

/// `AfterCombatVictory`: an elite room counts, and the fifth is
/// `RelicCmd.Replace` with Sword of Jade.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatVictory {
        room: RoomType::Elite,
    } = hook
    {
        state.elites_defeated += 1;
        if state.elites_defeated >= V::Relic::SWORD_OF_STONE_ELITES {
            asked.push(HookEffect::ReplaceRelic {
                from: V::Relic::SWORD_OF_STONE,
                to: V::Relic::SWORD_OF_JADE,
            });
        }
    }
}

/// `DisplayAmount`, which is `ElitesDefeated`.
pub(super) fn display_amount(state: &State) -> Number {
    state.elites_defeated
}
