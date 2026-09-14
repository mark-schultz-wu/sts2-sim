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
type Number = <V as sts2_core::ContentVersion>::Number;

/// `DivineRight`'s `StarsVar(3)`, which is the whole of what it gives.
const DIVINE_RIGHT_STARS: Number = 3;

/// `BoneTea._combatsLeft`, the one combat it opens with.
const BONE_TEA_COMBATS: Number = 1;

/// What a relic's own model saves, as the run picks it up.
pub(crate) fn counter(relic: Relic) -> Option<Number> {
    match relic {
        Relic::BoneTea => Some(BONE_TEA_COMBATS),
        _ => None,
    }
}

/// Apply a notification for one relic.
///
/// Divine Right is `AfterRoomEntered`, which pays its stars on the way into a
/// fight. Bone Tea is `AfterSideTurnStart`, which upgrades the hand the player
/// opens a fight with, and spends the combat it counted to do it. A used-up
/// Bone Tea returns early in the game, and here it matches no arm.
pub(crate) fn event_hook(relic: Relic, hook: EventHook<V>) -> Vec<HookEffect<V>> {
    match (relic, hook) {
        (Relic::DivineRight, EventHook::CombatRoomEntered) => {
            vec![HookEffect::GainStars(DIVINE_RIGHT_STARS)]
        }
        (
            Relic::BoneTea,
            EventHook::SideTurnStart {
                side: CombatSide::Player,
                turn: 1,
                counter: Some(combats),
            },
        ) if combats > 0 => vec![HookEffect::UpgradeHand, HookEffect::SetCounter(combats - 1)],
        _ => Vec::new(),
    }
}
