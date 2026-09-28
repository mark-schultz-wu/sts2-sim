//! `WarHammer`: four cards upgraded after each elite won.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::super::super::generated::runstate::RunRngType;

/// `CardsVar(4)`.
const CARDS: usize = 4;

/// `AfterCombatVictory` in an elite room: the deck's upgradable cards stably
/// shuffled off `RunState.Rng.Niche`, and the first four upgraded.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::CombatVictory {
        room: RoomType::Elite,
    } = hook
    {
        asked.push(HookEffect::UpgradeRandomDeckCards {
            count: CARDS,
            stream: RunRngType::Niche,
            kind: None,
        });
    }
}
