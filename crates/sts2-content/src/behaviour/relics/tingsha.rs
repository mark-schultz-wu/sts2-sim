//! `Tingsha`: a hit on a random enemy for each card its owner discards in
//! its own turn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

/// `AfterCardDiscarded` of its owner's card in its owner's side's turn:
/// `CreatureCmd.Damage` from its owner on an enemy that can be hit. The enemy
/// is drawn off `CombatTargets`.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardDiscarded {
            player,
            side: CombatSide::Player,
        } if player == owner => asked.push(HookEffect::DamageRandomOpponent {
            amount: V::Relic::TINGSHA_DAMAGE,
            unblockable: false,
            from_owner: true,
        }),
        EventHook::CardDiscarded { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what Tingsha does for {hook:?}"),
    }
}
