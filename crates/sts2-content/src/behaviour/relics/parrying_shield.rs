//! `ParryingShield`: a hit on a random enemy for a turn ended on enough
//! block.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

/// `AfterSideTurnEnd` on its owner's side, with at least its block held:
/// `CreatureCmd.Damage` from its owner on an enemy that can be hit, drawn
/// off `CombatTargets`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnEnd {
        side: CombatSide::Player,
        block,
    } = hook
        && block >= V::Relic::PARRYING_SHIELD_BLOCK
    {
        asked.push(HookEffect::DamageRandomOpponent {
            amount: V::Relic::PARRYING_SHIELD_DAMAGE,
            unblockable: false,
            from_owner: true,
        });
    }
}
