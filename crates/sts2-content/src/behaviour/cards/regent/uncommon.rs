//! The Regent's uncommon cards the generator leaves to a hand port.

use crate::ids::CardVarIds as _;
use sts2_core::{Change, CostPool, EventHook, HookEffect, Lasting};

/// `KinglyKick.AfterCardDrawn`: each time it is drawn, it costs one less for
/// the rest of the fight.
pub(super) fn kingly_kick<V: crate::Build>(
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
pub(super) fn kingly_punch<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    id: sts2_core::CardId<V>,
    printed: &P,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardDrawn { id: drawn, .. } if drawn == id => {
            asked.push(HookEffect::ModifySelf {
                change: Change::Own(crate::behaviour::cards::CardWrite::BuffFromPlay(
                    printed.face(V::CardVar::INCREASE),
                )),
                lasting: Lasting::ThisFight,
            })
        }
        EventHook::CardDrawn { .. } => {}
        hook => unimplemented!("what Kingly Punch does for {hook:?}"),
    }
}

/// `Radiate`: its damage to every enemy, once for each of its calculated
/// hits.
pub(super) fn radiate<V: crate::Build, F: sts2_core::Fight<V>>(
    fight: &mut F,
) -> crate::behaviour::Played<V, F::Kind> {
    let hits = fight.var(V::CardVar::CALCULATED_HITS);
    fight.attack(sts2_core::Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let targets = fight.opponents();
            // One `CreatureCmd.Damage` over them all.
            fight.damage_command(|fight| {
                for target in targets {
                    let attack = fight.resolve(sts2_core::Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: sts2_core::Amount::Var(V::CardVar::DAMAGE),
                        scaling: sts2_core::Scaling::Dynamic,
                    });
                    fight.apply(attack);
                }
            });
        }
    });
    crate::behaviour::Played::Done
}

/// `LunarBlast`: its `Damage` at its target, once for each of its
/// `CalculatedHits`.
pub(super) fn lunar_blast<V: crate::Build, F: sts2_core::Fight<V>>(
    fight: &mut F,
) -> crate::behaviour::Played<V, F::Kind> {
    let hits = fight.var(V::CardVar::CALCULATED_HITS);
    let target = fight.target();
    fight.attack(sts2_core::Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let attack = fight.resolve(sts2_core::Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: sts2_core::Amount::Var(V::CardVar::DAMAGE),
                scaling: sts2_core::Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    });
    crate::behaviour::Played::Done
}
