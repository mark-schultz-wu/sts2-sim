//! What an orb does, ported.
//!
//! `OrbModel`'s own triggers, one match per trigger, the way each orb's class
//! overrides them. An orb nobody has ported panics as it is channelled.

use crate::ids::OrbIds as _;
use sts2_core::{Amount, Channelled, CreatureView as _, Effect, Fight, Scaling, Table as _};

/// What an orb's numbers are held as, in every build.
type Number = i32;

/// A number the table holds for an orb.
fn printed(amount: Option<nonmax::NonMaxU8>) -> Number {
    Number::from(amount.expect("the orb's getter names a number").get())
}

/// An orb as it joins the queue.
///
/// # Panics
///
/// An orb nobody has ported.
pub(crate) fn channelled<V: crate::Build>(orb: V::Orb) -> Channelled<V> {
    match orb {
        __c0 if __c0 == V::Orb::LIGHTNING_ORB
            || __c0 == V::Orb::FROST_ORB
            || __c0 == V::Orb::PLASMA_ORB
            || __c0 == V::Orb::DARK_ORB =>
        {
            let data = orb.data();
            Channelled {
                orb,
                passive: printed(data.passive.amount),
                evoke: printed(data.evoke.amount),
            }
        }
        // `GlassOrb.EvokeVal` is twice its passive, which the getter works out.
        __c0 if __c0 == V::Orb::GLASS_ORB => {
            let passive = printed(orb.data().passive.amount);
            Channelled {
                orb,
                passive,
                evoke: passive * 2,
            }
        }
        orb => unimplemented!("channelling {orb:?}"),
    }
}

/// `BeforeTurnEndOrbTrigger`.
pub(crate) fn turn_end<V: crate::Build, F: Fight<V>>(orb: &mut Channelled<V>, fight: &mut F) {
    match orb.orb {
        __c0 if __c0 == V::Orb::LIGHTNING_ORB
            || __c0 == V::Orb::FROST_ORB
            || __c0 == V::Orb::DARK_ORB
            || __c0 == V::Orb::GLASS_ORB =>
        {
            passive(orb, fight)
        }
        __c0 if __c0 == V::Orb::PLASMA_ORB => {}
        orb => unimplemented!("what {orb:?} does as a turn ends"),
    }
}

/// `Passive(null)`. An orb whose numbers move writes them on `orb`.
pub(crate) fn passive<V: crate::Build, F: Fight<V>>(orb: &mut Channelled<V>, fight: &mut F) {
    match orb.orb {
        // Its passive number to a random enemy.
        __c0 if __c0 == V::Orb::LIGHTNING_ORB => {
            lightning(passive_val(orb, |n| fight.orb_value(orb.orb, n)), fight);
        }
        __c0 if __c0 == V::Orb::FROST_ORB => {
            frost(passive_val(orb, |n| fight.orb_value(orb.orb, n)), fight)
        }
        // `PlayerCmd.GainEnergy` by its passive.
        __c0 if __c0 == V::Orb::PLASMA_ORB => {
            fight.modify_energy(passive_val(orb, |n| fight.orb_value(orb.orb, n)))
        }
        // `_evokeVal += PassiveVal`.
        __c0 if __c0 == V::Orb::DARK_ORB => {
            orb.evoke += passive_val(orb, |n| fight.orb_value(orb.orb, n))
        }
        // Its passive to every enemy that can be hit, the list taken first,
        // and one off what it deals next while it deals anything.
        __c0 if __c0 == V::Orb::GLASS_ORB => {
            let targets = fight.hittable_opponents();
            let value = passive_val(orb, |n| fight.orb_value(orb.orb, n));
            if value > 0 {
                orb.passive = (orb.passive - 1).max(0);
                orb.evoke = orb.passive * 2;
                // One `CreatureCmd.Damage` over them all.
                fight.damage_command(|fight| {
                    for target in targets {
                        unpowered(value, target, fight);
                    }
                });
            }
        }
        orb => unimplemented!("what {orb:?} does as its passive"),
    }
}

/// `OrbCmd.Passive(orb, target)`, which a card aims. Only a Lightning reads
/// the target.
///
/// # Panics
///
/// Any other orb.
pub(crate) fn passive_at<V: crate::Build, F: Fight<V>>(
    orb: &Channelled<V>,
    target: sts2_core::CombatId,
    fight: &mut F,
) {
    match orb.orb {
        // Its passive number at the target, while any enemy can be hit.
        __c0 if __c0 == V::Orb::LIGHTNING_ORB => {
            let value = passive_val(orb, |n| fight.orb_value(orb.orb, n));
            if !fight.hittable_opponents().is_empty() {
                let hit = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Number(value),
                    scaling: Scaling::Static,
                });
                fight.apply(hit);
            }
        }
        orb => unimplemented!("{orb:?}'s passive aimed at a creature"),
    }
}

/// `AfterTurnStartOrbTrigger`, which the base orb leaves empty.
pub(crate) fn turn_start<V: crate::Build, F: Fight<V>>(orb: &mut Channelled<V>, fight: &mut F) {
    match orb.orb {
        __c0 if __c0 == V::Orb::LIGHTNING_ORB
            || __c0 == V::Orb::FROST_ORB
            || __c0 == V::Orb::DARK_ORB
            || __c0 == V::Orb::GLASS_ORB => {}
        // `PlasmaOrb.AfterTurnStartOrbTrigger`, which is its passive.
        __c0 if __c0 == V::Orb::PLASMA_ORB => passive(orb, fight),
        orb => unimplemented!("what {orb:?} does as a turn starts"),
    }
}

/// `Evoke`.
///
/// The creatures it hit, which `Evoke` returns.
pub(crate) fn evoke<V: crate::Build, F: Fight<V>>(
    orb: &Channelled<V>,
    fight: &mut F,
) -> sts2_core::Opponents {
    match orb.orb {
        __c0 if __c0 == V::Orb::LIGHTNING_ORB => {
            lightning(evoke_val(orb, |n| fight.orb_value(orb.orb, n)), fight)
                .into_iter()
                .collect()
        }
        // Its evoke value as block on its owner, the one creature it names.
        __c0 if __c0 == V::Orb::FROST_ORB => {
            frost(evoke_val(orb, |n| fight.orb_value(orb.orb, n)), fight);
            std::iter::once(fight.actor()).collect()
        }
        // Its evoke as energy, and its owner the one creature it names.
        __c0 if __c0 == V::Orb::PLASMA_ORB => {
            fight.modify_energy(evoke_val(orb, |n| fight.orb_value(orb.orb, n)));
            std::iter::once(fight.actor()).collect()
        }
        // Its evoke at the enemy with the least health, the first of them.
        __c0 if __c0 == V::Orb::DARK_ORB => {
            let weakest = fight
                .hittable_opponents()
                .into_iter()
                .filter_map(|id| {
                    fight
                        .creatures()
                        .find(|creature| creature.id() == id)
                        .map(|creature| (creature.hp(), id))
                })
                .reduce(|least, next| if next.0 < least.0 { next } else { least });
            let Some((_, target)) = weakest else {
                return sts2_core::Opponents::new();
            };
            unpowered(
                evoke_val(orb, |n| fight.orb_value(orb.orb, n)),
                target,
                fight,
            );
            std::iter::once(target).collect()
        }
        // Its evoke to every enemy that can be hit, while it deals anything.
        __c0 if __c0 == V::Orb::GLASS_ORB => {
            let targets = fight.hittable_opponents();
            let value = evoke_val(orb, |n| fight.orb_value(orb.orb, n));
            if value <= 0 {
                return sts2_core::Opponents::new();
            }
            // One `CreatureCmd.Damage` over them all.
            fight.damage_command(|fight| {
                for &target in &targets {
                    unpowered(value, target, fight);
                }
            });
            targets.into_iter().collect()
        }
        orb => unimplemented!("what {orb:?} does as it is evoked"),
    }
}

/// `PassiveVal`. `modify` is `ModifyOrbValue`.
pub(crate) fn passive_val<V: crate::Build>(
    orb: &Channelled<V>,
    modify: impl FnOnce(Number) -> Number,
) -> Number {
    match orb.orb {
        __c0 if __c0 == V::Orb::LIGHTNING_ORB
            || __c0 == V::Orb::FROST_ORB
            || __c0 == V::Orb::DARK_ORB
            || __c0 == V::Orb::GLASS_ORB =>
        {
            modify(orb.passive)
        }
        // `PlasmaOrb.PassiveVal`, its number as printed.
        __c0 if __c0 == V::Orb::PLASMA_ORB => orb.passive,
        orb => unimplemented!("{orb:?}'s passive value"),
    }
}

/// `EvokeVal`. `modify` is `ModifyOrbValue`.
pub(crate) fn evoke_val<V: crate::Build>(
    orb: &Channelled<V>,
    modify: impl FnOnce(Number) -> Number,
) -> Number {
    match orb.orb {
        __c0 if __c0 == V::Orb::LIGHTNING_ORB || __c0 == V::Orb::FROST_ORB => modify(orb.evoke),
        // `PlasmaOrb.EvokeVal`, and `DarkOrb`'s, their numbers as held.
        __c0 if __c0 == V::Orb::PLASMA_ORB || __c0 == V::Orb::DARK_ORB => orb.evoke,
        // `GlassOrb.EvokeVal`: twice `PassiveVal`.
        __c0 if __c0 == V::Orb::GLASS_ORB => modify(orb.passive) * 2,
        orb => unimplemented!("{orb:?}'s evoke value"),
    }
}

/// `CreatureCmd.Damage` of `value` on `target`, `ValueProp.Unpowered`, from
/// the player.
fn unpowered<V: crate::Build, F: Fight<V>>(
    value: Number,
    target: sts2_core::CombatId,
    fight: &mut F,
) {
    let hit = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target,
        damage: Amount::Number(value),
        scaling: Scaling::Static,
    });
    fight.apply(hit);
}

/// `FrostOrb`'s block: `value` on the player, unpowered.
fn frost<V: crate::Build, F: Fight<V>>(value: Number, fight: &mut F) {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Number(value),
        scaling: Scaling::Static,
    });
    fight.apply(block);
}

/// `LightningOrb.ApplyLightningDamage` with no target named: one enemy that
/// can be hit, drawn off `CombatTargets`, takes `value`, `ValueProp.Unpowered`
/// from the player. Nothing with no such enemy, and no draw. The one it hit.
fn lightning<V: crate::Build, F: Fight<V>>(
    value: Number,
    fight: &mut F,
) -> Option<sts2_core::CombatId> {
    let target = fight.random_opponent()?;
    let hit = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target,
        damage: Amount::Number(value),
        scaling: Scaling::Static,
    });
    fight.apply(hit);
    Some(target)
}
