//! `ParryingShield`: a hit on a random enemy for a turn ended on enough
//! block.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `BlockVar(10, ValueProp.Unpowered)`, the block the turn ends on at least.
const BLOCK: Number = 10;

/// `DamageVar(6, ValueProp.Unpowered)`.
const DAMAGE: Number = 6;

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
        && block >= BLOCK
    {
        asked.push(HookEffect::DamageRandomOpponent {
            amount: DAMAGE,
            unblockable: false,
            from_owner: true,
        });
    }
}
