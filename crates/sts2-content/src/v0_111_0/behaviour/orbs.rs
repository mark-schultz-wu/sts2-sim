//! The orbs whose rules this build ports for itself, which the shared orbs
//! hand over through [`crate::build::OwnRules`].

use sts2_core::{Amount, CreatureView as _, Effect, Fight, Scaling};

use super::super::V0_111_0 as V;
use super::super::generated::orbs::Orb;
use super::super::generated::powers::Power;
use crate::behaviour::orbs::{evoke_val, passive_val};

/// `FrostOrb.Passive(null)`: its passive as block on its owner. While the
/// owner has Hibernate, the same on every other player.
pub(crate) fn own_passive<F: Fight<V>>(
    orb: &mut sts2_core::Channelled<V>,
    fight: &mut F,
) -> Option<()> {
    match orb.orb {
        Orb::FrostOrb => {
            let value = passive_val(orb, |n| fight.orb_value(orb.orb, n));
            frost(value, fight);
            Some(())
        }
        _ => None,
    }
}

/// `FrostOrb.Evoke`: its evoke as block on its owner, who is the one
/// creature it names. While the owner has Hibernate, the same on every
/// other player, and it names every player.
pub(crate) fn own_evoke<F: Fight<V>>(
    orb: &sts2_core::Channelled<V>,
    fight: &mut F,
) -> Option<sts2_core::Opponents> {
    match orb.orb {
        Orb::FrostOrb => {
            let value = evoke_val(orb, |n| fight.orb_value(orb.orb, n));
            Some(match frost(value, fight) {
                true => fight.players().into_iter().collect(),
                false => std::iter::once(fight.actor()).collect(),
            })
        }
        _ => None,
    }
}

/// `value` as unpowered block on the orb's owner, then on every other
/// player, the dead too, while the owner has Hibernate. Answers whether the
/// owner had it.
fn frost<F: Fight<V>>(value: i32, fight: &mut F) -> bool {
    let owner = fight.actor();
    block(owner, value, fight);
    let hibernating = (fight.creatures())
        .find(|creature| creature.id() == owner)
        .is_some_and(|creature| creature.power(Power::HibernatePower).is_some());
    if hibernating {
        for player in fight.players() {
            if player != owner {
                block(player, value, fight);
            }
        }
    }
    hibernating
}

/// `CreatureCmd.GainBlock(recipient, value, ValueProp.Unpowered, null)`.
fn block<F: Fight<V>>(recipient: sts2_core::CombatId, value: i32, fight: &mut F) {
    let block = fight.resolve(Effect::GainBlock {
        recipient,
        amount: Amount::Number(value),
        scaling: Scaling::Static,
    });
    fight.apply(block);
}
