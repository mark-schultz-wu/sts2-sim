//! The Regent's uncommon cards the generator leaves to a hand port.

use sts2_core::{Change, CostPool, EventHook, HookEffect, Lasting};

use super::V;

/// `KinglyKick.AfterCardDrawn`: each time it is drawn, it costs one less for
/// the rest of the fight.
pub(super) fn kingly_kick(
    id: sts2_core::CardId<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardDrawn { id: drawn, .. } if drawn == id => {
            asked.push(HookEffect::ModifySelf {
                change: Change::ChangeCost(CostPool::Energy, -1),
                lasting: Lasting::ThisFight,
            })
        }
        EventHook::CardDrawn { .. } => {}
        hook => unimplemented!("what Kingly Kick does for {hook:?}"),
    }
}
