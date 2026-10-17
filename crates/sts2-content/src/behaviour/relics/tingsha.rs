//! `Tingsha`: a hit on a random enemy for each card its owner discards in
//! its own turn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

/// `AfterCardDiscarded` in its owner's side's turn: `CreatureCmd.Damage` from
/// its owner on an enemy that can be hit, drawn off `CombatTargets`. A fight
/// here has one player, whose every card is its owner's.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardDiscarded {
            side: CombatSide::Player,
        } => asked.push(HookEffect::DamageRandomOpponent {
            amount: V::Relic::TINGSHA_DAMAGE,
            unblockable: false,
            from_owner: true,
        }),
        EventHook::CardDiscarded { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what Tingsha does for {hook:?}"),
    }
}
