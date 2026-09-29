//! `Tingsha`: a hit on a random enemy for each card its owner discards in
//! its own turn.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `DamageVar(3, ValueProp.Unpowered)`.
const DAMAGE: Number = 3;

/// `AfterCardDiscarded` in its owner's side's turn: `CreatureCmd.Damage` from
/// its owner on an enemy that can be hit, drawn off `CombatTargets`. A fight
/// here has one player, whose every card is its owner's.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CardDiscarded {
            side: CombatSide::Player,
        } => asked.push(HookEffect::DamageRandomOpponent {
            amount: DAMAGE,
            unblockable: false,
            from_owner: true,
        }),
        EventHook::CardDiscarded { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what Tingsha does for {hook:?}"),
    }
}
