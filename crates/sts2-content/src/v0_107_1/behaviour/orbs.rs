//! What an orb does, ported.
//!
//! `OrbModel`'s own triggers, one match per trigger, the way each orb's class
//! overrides them. An orb nobody has ported panics as it is channelled.

use sts2_core::{Amount, Channelled, Effect, Fight, Scaling};

use super::super::V0_107_1 as V;
use super::super::generated::orbs::Orb;

/// What an orb's numbers are held as.
type Number = <V as sts2_core::ContentVersion>::Number;

/// A number the table holds for an orb.
fn printed(amount: Option<nonmax::NonMaxU8>) -> Number {
    Number::from(amount.expect("the orb's getter names a number").get())
}

/// An orb as it joins the queue.
///
/// # Panics
///
/// An orb nobody has ported.
pub(crate) fn channelled(orb: Orb) -> Channelled<V> {
    match orb {
        Orb::LightningOrb | Orb::FrostOrb | Orb::PlasmaOrb => {
            let data = orb.data();
            Channelled {
                orb,
                passive: printed(data.passive.amount),
                evoke: printed(data.evoke.amount),
            }
        }
        orb => unimplemented!("channelling {orb:?}"),
    }
}

/// `BeforeTurnEndOrbTrigger`.
pub(crate) fn turn_end<F: Fight<V>>(orb: &Channelled<V>, fight: &mut F) {
    match orb.orb {
        Orb::LightningOrb | Orb::FrostOrb => passive(orb, fight),
        Orb::PlasmaOrb => {}
        orb => unimplemented!("what {orb:?} does as a turn ends"),
    }
}

/// `Passive(null)`.
pub(crate) fn passive<F: Fight<V>>(orb: &Channelled<V>, fight: &mut F) {
    match orb.orb {
        // Its passive number to a random enemy.
        Orb::LightningOrb => {
            lightning(passive_val(orb, |n| fight.orb_value(orb.orb, n)), fight);
        }
        Orb::FrostOrb => frost(passive_val(orb, |n| fight.orb_value(orb.orb, n)), fight),
        // `PlayerCmd.GainEnergy` by its passive.
        Orb::PlasmaOrb => fight.modify_energy(passive_val(orb, |n| fight.orb_value(orb.orb, n))),
        orb => unimplemented!("what {orb:?} does as its passive"),
    }
}

/// `OrbCmd.Passive(orb, target)`, which a card aims. Only a Lightning reads
/// the target.
///
/// # Panics
///
/// Any other orb.
pub(crate) fn passive_at<F: Fight<V>>(
    orb: &Channelled<V>,
    target: sts2_core::CombatId,
    fight: &mut F,
) {
    match orb.orb {
        // Its passive number at the target, while any enemy can be hit.
        Orb::LightningOrb => {
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
pub(crate) fn turn_start<F: Fight<V>>(orb: &Channelled<V>, fight: &mut F) {
    match orb.orb {
        Orb::LightningOrb | Orb::FrostOrb => {}
        // `PlasmaOrb.AfterTurnStartOrbTrigger`, which is its passive.
        Orb::PlasmaOrb => passive(orb, fight),
        orb => unimplemented!("what {orb:?} does as a turn starts"),
    }
}

/// `Evoke`.
///
/// The creatures it hit, which `Evoke` returns.
pub(crate) fn evoke<F: Fight<V>>(orb: &Channelled<V>, fight: &mut F) -> sts2_core::Opponents {
    match orb.orb {
        Orb::LightningOrb => lightning(evoke_val(orb, |n| fight.orb_value(orb.orb, n)), fight)
            .into_iter()
            .collect(),
        // Its evoke value as block on its owner, the one creature it names.
        Orb::FrostOrb => {
            frost(evoke_val(orb, |n| fight.orb_value(orb.orb, n)), fight);
            std::iter::once(fight.actor()).collect()
        }
        // Its evoke as energy, and its owner the one creature it names.
        Orb::PlasmaOrb => {
            fight.modify_energy(evoke_val(orb, |n| fight.orb_value(orb.orb, n)));
            std::iter::once(fight.actor()).collect()
        }
        orb => unimplemented!("what {orb:?} does as it is evoked"),
    }
}

/// `PassiveVal`. `modify` is `ModifyOrbValue`.
pub(crate) fn passive_val(orb: &Channelled<V>, modify: impl FnOnce(Number) -> Number) -> Number {
    match orb.orb {
        Orb::LightningOrb | Orb::FrostOrb => modify(orb.passive),
        // `PlasmaOrb.PassiveVal`, its number as printed.
        Orb::PlasmaOrb => orb.passive,
        orb => unimplemented!("{orb:?}'s passive value"),
    }
}

/// `EvokeVal`. `modify` is `ModifyOrbValue`.
pub(crate) fn evoke_val(orb: &Channelled<V>, modify: impl FnOnce(Number) -> Number) -> Number {
    match orb.orb {
        Orb::LightningOrb | Orb::FrostOrb => modify(orb.evoke),
        // `PlasmaOrb.EvokeVal`, its number as printed.
        Orb::PlasmaOrb => orb.evoke,
        orb => unimplemented!("{orb:?}'s evoke value"),
    }
}

/// `FrostOrb`'s block: `value` on the player, unpowered.
fn frost<F: Fight<V>>(value: Number, fight: &mut F) {
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
fn lightning<F: Fight<V>>(value: Number, fight: &mut F) -> Option<sts2_core::CombatId> {
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
