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

/// `Radiate`: its damage to every enemy, once for each of its calculated
/// hits.
pub(super) fn radiate<F: sts2_core::Fight<V>>(fight: &mut F) -> sts2_core::Played<V> {
    let hits = fight.var(CardVar::CalculatedHits);
    fight.attack(sts2_core::Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            for target in fight.opponents() {
                let attack = fight.resolve(sts2_core::Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: sts2_core::Amount::Var(CardVar::Damage),
                    scaling: sts2_core::Scaling::Dynamic,
                });
                fight.apply(attack);
            }
        }
    });
    sts2_core::Played::Done
}

/// `LunarBlast`: its `Damage` at its target, once for each of its
/// `CalculatedHits`.
pub(super) fn lunar_blast<F: sts2_core::Fight<V>>(fight: &mut F) -> sts2_core::Played<V> {
    let hits = fight.var(CardVar::CalculatedHits);
    let target = fight.target();
    fight.attack(sts2_core::Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let attack = fight.resolve(sts2_core::Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: sts2_core::Amount::Var(CardVar::Damage),
                scaling: sts2_core::Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    });
    sts2_core::Played::Done
}
