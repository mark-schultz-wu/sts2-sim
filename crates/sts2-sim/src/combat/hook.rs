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
            // `CreatureCmd.Heal` on the listener's owner, the actor.
            HookEffect::Heal(amount) => fight.heal(fight.actor(), amount),
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
            HookEffect::Afflict { to, affliction } => fight.afflict(to, affliction),
            HookEffect::ClearAffliction(to) => fight.clear_affliction(to),
            HookEffect::ChannelRandomOrbs { orbs, count } => {
                let count: usize = num_traits::cast(count)
                    .unwrap_or_else(|| unimplemented!("{count:?} orbs channelled"));
                for _ in 0..count {
                    if let Some(orb) = fight.random_orb(orbs) {
                        fight.channel(orb);
                    }
                }
            }
            HookEffect::FreeCostlyInHand => {
                use sts2_core::Cost;

                let hand = fight.cards_in(sts2_core::Pile::Hand);
                let positive = |cost: Cost| matches!(cost, Cost::Fixed(n) if n.get() > 0);
                let prints: Vec<_> = hand
                    .iter()
                    .copied()
                    .filter(|&at| {
                        let (energy, stars) = fight.printed_costs(at);
                        positive(energy) || positive(stars)
                    })
                    .collect();
                let costly = |at: &sts2_core::CardAt<V>| {
                    let (energy, stars) = fight.card_costs(*at, true);
                    positive(energy) || positive(stars)
                };
                let stream = <V::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_SELECTION;
                let both: Vec<_> = prints.iter().copied().filter(costly).collect();
                let now: Vec<_> = hand.iter().copied().filter(costly).collect();
                let picked = [both, now, prints, hand]
                    .into_iter()
                    .find_map(|among| fight.random_card(&among, stream));
                if let Some(at) = picked {
                    for pool in [sts2_core::CostPool::Energy, sts2_core::CostPool::Star] {
                        fight.modify_card(
                            at,
                            sts2_core::Change::SetCost(pool, num_traits::zero()),
                            sts2_core::Lasting::ThisTurnOrUntilPlayed,
                        );
                    }
                }
            }
            other => unimplemented!("a model asking for {other:?} inside a fight"),
        }
    }
}
