//! What a relic does, ported.
//!
//! One function per hook, matching on the relic the way `RelicModel`'s
//! subclasses override one. A relic gets an arm when a run can carry it into
//! the state the hook fires from.
//!
//! A relic with no arm asks for nothing. A run carries every relic it picks
//! up, so an unported one sits in the belt and does nothing. The golden replay
//! is what catches the fight where that silence is wrong.

use sts2_core::{EventHook, HookEffect};

use super::super::V0_107_1 as V;
use super::super::generated::relics::Relic;
use super::super::generated::runstate::CombatSide;

/// Apply a notification for one relic.
///
/// Bone Tea is `AfterSideTurnStart`, which upgrades the hand the player opens
/// a fight with. The game also counts the relic down to one combat and
/// disables it. No observation carries that counter, so a second fight with
/// Bone Tea still held upgrades again.
pub(crate) fn event_hook(relic: Relic, hook: EventHook<V>) -> Vec<HookEffect<V>> {
    match (relic, hook) {
        (
            Relic::BoneTea,
            EventHook::SideTurnStart {
                side: CombatSide::Player,
                turn: 1,
            },
        ) => vec![HookEffect::UpgradeHand],
        _ => Vec::new(),
    }
}
