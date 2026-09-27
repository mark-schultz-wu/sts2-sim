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
        Orb::LightningOrb | Orb::FrostOrb => {
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
        orb => unimplemented!("what {orb:?} does as a turn ends"),
    }
}

/// `Passive(null)`.
pub(crate) fn passive<F: Fight<V>>(orb: &Channelled<V>, fight: &mut F) {
    match orb.orb {
        // Its passive number to a random enemy.
        Orb::LightningOrb => {
            lightning(fight.orb_value(orb.orb, orb.passive), fight);
        }
        Orb::FrostOrb => frost(fight.orb_value(orb.orb, orb.passive), fight),
        orb => unimplemented!("what {orb:?} does as its passive"),
    }
}

/// `AfterTurnStartOrbTrigger`, which the base orb leaves empty.
pub(crate) fn turn_start<F: Fight<V>>(orb: &Channelled<V>, _fight: &mut F) {
    match orb.orb {
        Orb::LightningOrb | Orb::FrostOrb => {}
        orb => unimplemented!("what {orb:?} does as a turn starts"),
    }
}

/// `Evoke`.
///
/// The creatures it hit, which `Evoke` returns.
pub(crate) fn evoke<F: Fight<V>>(orb: &Channelled<V>, fight: &mut F) -> sts2_core::Opponents {
    match orb.orb {
        Orb::LightningOrb => lightning(fight.orb_value(orb.orb, orb.evoke), fight)
            .into_iter()
            .collect(),
        // Its evoke value as block on its owner, the one creature it names.
        Orb::FrostOrb => {
            frost(fight.orb_value(orb.orb, orb.evoke), fight);
            std::iter::once(fight.actor()).collect()
        }
        orb => unimplemented!("what {orb:?} does as it is evoked"),
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
