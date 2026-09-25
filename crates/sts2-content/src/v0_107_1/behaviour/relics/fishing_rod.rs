//! `FishingRod`: a random card upgraded every third monster fight.

use sts2_core::{EventHook, HookEffect, Stream};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::super::super::generated::runstate::RunRngType;
use super::Number;

/// `DynamicVar("Combats", 3)`.
const COMBATS: Number = 3;

/// `CombatsSeen`, the one `[SavedProperty]` this relic declares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    combats_seen: Number,
}

/// `AfterCombatEnd`: a monster room counts, and every third one upgrades a
/// random upgradable card of the deck off `RunState.Rng.Niche`.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatEnd {
        room: RoomType::Monster,
    } = hook
    {
        state.combats_seen += 1;
        if state.combats_seen % COMBATS == 0 {
            asked.push(HookEffect::UpgradeRandomDeckCard {
                stream: Stream::Run(RunRngType::Niche),
            });
        }
    }
}

/// `DisplayAmount`, which is `CombatsSeen % Combats`.
pub(super) fn display_amount(state: &State) -> Number {
    state.combats_seen % COMBATS
}
