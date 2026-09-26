//! The Regent's uncommon cards the generator leaves to a hand port.

use sts2_core::{Change, CostPool, EventHook, HookEffect, Lasting};

use super::super::super::super::generated::cards::CardVar;

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

/// `KinglyPunch.AfterCardDrawn`: each time it is drawn, its damage rises by
/// its `Increase` for the rest of the fight.
pub(super) fn kingly_punch<P: sts2_core::Printed<V> + ?Sized>(
    id: sts2_core::CardId<V>,
    printed: &P,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardDrawn { id: drawn, .. } if drawn == id => {
            asked.push(HookEffect::ModifySelf {
                change: Change::Raise(CardVar::Damage, printed.face(CardVar::Increase)),
                lasting: Lasting::ThisFight,
            })
        }
        EventHook::CardDrawn { .. } => {}
        hook => unimplemented!("what Kingly Punch does for {hook:?}"),
    }
}
