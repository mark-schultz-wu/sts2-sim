//! `WarHammer`: four cards upgraded after each elite won.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::encounters::RoomType;
use crate::generated::runstate::RunRngType;

/// `AfterCombatVictory` in an elite room: the deck's upgradable cards stably
/// shuffled off `RunState.Rng.Niche`, and the first four upgraded.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CombatVictory {
        room: RoomType::Elite,
    } = hook
    {
        asked.push(HookEffect::UpgradeRandomDeckCards {
            count: (V::Relic::WAR_HAMMER_CARDS as usize),
            stream: sts2_core::Stream::Run(RunRngType::Niche),
            kind: None,
        });
    }
}
