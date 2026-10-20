//! `FishingRod`: a random card upgraded every third monster fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Stream};

use crate::behaviour::relics::Number;
use crate::generated::encounters::RoomType;
use crate::generated::runstate::RunRngType;

/// `CombatsSeen`, the one `[SavedProperty]` this relic declares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    combats_seen: Number,
}

/// `AfterCombatEnd`: a monster room counts, and every third one upgrades a
/// random upgradable card of the deck off `RunState.Rng.Niche`.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatEnd {
        room: RoomType::Monster,
    } = hook
    {
        state.combats_seen += 1;
        if state.combats_seen % V::Relic::FISHING_ROD_COMBATS == 0 {
            asked.push(HookEffect::UpgradeRandomDeckCard {
                stream: Stream::Run(RunRngType::Niche),
            });
        }
    }
}

/// `DisplayAmount`, which is `CombatsSeen % Combats`.
pub(super) fn display_amount<V: crate::Build>(state: &State) -> Number {
    state.combats_seen % V::Relic::FISHING_ROD_COMBATS
}
