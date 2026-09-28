//! What a model asked for, carried out against the fight.
//!
//! `Hook` hands each listener the notification and takes back what it wants
//! done. A power, a relic and a card all answer in the same vocabulary, so
//! this is where that vocabulary becomes commands, whichever kind asked.

use sts2_core::{Fight, HookEffect, Rules};

/// Carry out what a model asked for, through the fight it asked in.
///
/// # Panics
///
/// An effect no fight can carry out here.
pub(crate) fn apply<V: Rules, F: Fight<V>>(
    fight: &mut F,
    asked: impl IntoIterator<Item = HookEffect<V>>,
) {
    for effect in asked {
        match effect {
            HookEffect::GainEnergy(amount) => fight.modify_energy(amount),
            HookEffect::GainEnergyForEachPlay { card, energy } => {
                let plays = fight.generate_play_count(card);
                for _ in 0..num_traits::ToPrimitive::to_usize(&plays).unwrap_or(0) {
                    fight.modify_energy(energy);
                }
            }
            HookEffect::LoseEnergy(amount) => fight.modify_energy(-amount),
            HookEffect::Draw(amount) => fight.draw(amount),
            HookEffect::GainStars(amount) => fight.gain_stars(amount),
            HookEffect::GainBlock(amount) => {
                let block = fight.resolve(sts2_core::Effect::GainBlock {
                    recipient: fight.actor(),
                    amount: sts2_core::Amount::Number(amount),
                    scaling: sts2_core::Scaling::Static,
                });
                fight.apply(block);
            }
            HookEffect::DamageUnpowered { target, amount } => {
                fight.damage_unpowered(None, target, amount, false);
            }
            HookEffect::GainBlockOf { target, amount } => {
                let block = fight.resolve(sts2_core::Effect::GainBlock {
                    recipient: target,
                    amount: sts2_core::Amount::Number(amount),
                    scaling: sts2_core::Scaling::Static,
                });
                fight.apply(block);
            }
            // `PowerCmd.Apply` on the creature carrying the listener. Every
            // hook delivered here names the creature acting, and a power only
            // answers one that its own owner is party to, so the two are the
            // same creature. `GainBlock` above reads the actor for that
            // reason. A listener that could answer for someone else would need
            // the owner carried alongside the effect.
            HookEffect::ApplyToOwner(power, amount) => fight.apply_to_actor(power, amount),
            // `PowerCmd.Apply<P>(HittableEnemies, amount, Owner)`, the list
            // taken before the first lands.
            HookEffect::ApplyToOpponents(power, amount) => {
                for target in fight.hittable_opponents() {
                    fight.apply_power(target, power, amount);
                }
            }
            // `CreatureCmd.Damage` over `HittableEnemies`, unpowered and from
            // the owner.
            HookEffect::DamageOpponents(amount) => {
                for target in fight.hittable_opponents() {
                    let hit = fight.resolve(sts2_core::Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: sts2_core::Amount::Number(amount),
                        scaling: sts2_core::Scaling::Static,
                    });
                    fight.apply(hit);
                }
            }
            // `CreatureCmd.Damage` on one of `HittableEnemies` drawn off
            // `CombatTargets`, unpowered.
            HookEffect::DamageRandomOpponent {
                amount,
                unblockable,
                from_owner,
            } => {
                if let Some(target) = fight.random_opponent() {
                    let dealer = from_owner.then(|| fight.actor());
                    fight.damage_unpowered(dealer, target, amount, unblockable);
                }
            }
            other => unimplemented!("a model asking for {other:?} inside a fight"),
        }
    }
}
