//! The Necrobinder's cards, where the generator leaves them.

use sts2_core::{
    Amount, Change, CostPool, Effect, EventHook, Fight, HookEffect, Lasting, Played, Scaling,
};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, NecrobinderCard};
use super::super::super::generated::monsters::Monster;
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: NecrobinderCard, fight: &mut F) -> Played<V> {
    match card {
        NecrobinderCard::Unleash => osty_attack(fight, CardVar::CalculatedDamage),
        NecrobinderCard::Poke | NecrobinderCard::Flatten => osty_attack(fight, CardVar::OstyDamage),
        card => card_plays::necrobinder::on_play(card, fight),
    }
}

/// `DamageCmd.Attack(damage).FromOsty(Owner.Osty, this).Targeting(target)`,
/// unless `Osty.CheckMissingWithAnim` finds no Osty. Osty is the attacker, so
/// the hooks an attack raises name Osty, and the card is what they read as
/// the source.
fn osty_attack<F: Fight<V>>(fight: &mut F, damage: CardVar) -> Played<V> {
    let Some(osty) = fight.osty() else {
        return Played::Done;
    };
    let target = fight.target();
    fight.attack_from(osty, Scaling::Dynamic, |fight| {
        let hit = fight.resolve(Effect::Attack {
            dealer: osty,
            target,
            damage: Amount::Var(damage),
            scaling: Scaling::Dynamic,
        });
        fight.apply(hit);
    });
    Played::Done
}

/// Deliver a notification to one Necrobinder card in the fight.
pub(super) fn event_hook<P: sts2_core::Printed<V> + ?Sized>(
    card: NecrobinderCard,
    id: sts2_core::CardId<V>,
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (card, hook) {
        // `Flatten.AfterAttack`, which answers an attack by its owner's Osty
        // alone. A fight here has one player, so any Osty is theirs.
        (
            NecrobinderCard::Flatten,
            EventHook::AfterAttack {
                monster: Some(Monster::Osty),
                ..
            },
        ) => asked.push(HookEffect::ModifySelf {
            change: Change::SetCost(CostPool::Energy, 0),
            lasting: Lasting::ThisTurn,
        }),
        (NecrobinderCard::Flatten, EventHook::AfterAttack { .. }) => {}
        // `Flatten.AfterCardEnteredCombat`, which answers its own entry
        // alone, and then only once Osty has attacked this turn.
        (NecrobinderCard::Flatten, EventHook::CardEnteredCombat { at, .. }) => {
            if at.id() == id {
                unimplemented!("a Flatten entering the fight, which reads whether Osty attacked")
            }
        }
        // `Melancholy.AfterDeath`: any creature's death takes its `Energy` off
        // its cost for the rest of the fight. The sim prevents no death, so
        // every one counts, and a card that hears a hook is in a combat pile.
        (NecrobinderCard::Melancholy, EventHook::AfterDeath { .. }) => {
            asked.push(HookEffect::ModifySelf {
                change: Change::ChangeCost(CostPool::Energy, -printed.face(CardVar::Energy)),
                lasting: Lasting::ThisFight,
            })
        }
        // `BansheesCry.AfterCardPlayed`: its `Energy` off its cost for the
        // rest of the fight for each Ethereal card its owner plays. A fight
        // here has one player.
        (NecrobinderCard::BansheesCry, EventHook::CardPlayed { ethereal: true, .. }) => {
            asked.push(HookEffect::ModifySelf {
                change: Change::ChangeCost(CostPool::Energy, -printed.face(CardVar::Energy)),
                lasting: Lasting::ThisFight,
            })
        }
        (NecrobinderCard::BansheesCry, EventHook::CardPlayed { .. }) => {}
        // `BansheesCry.AfterCardEnteredCombat`, for its own entry alone and
        // not a clone's: its `Energy` off for the rest of the fight for each
        // Ethereal play finished this fight before it came.
        (NecrobinderCard::BansheesCry, EventHook::CardEnteredCombat { at, clone, .. }) => {
            if at.id() == id && !clone {
                let ethereal: super::Number = history
                    .finished_this_fight()
                    .filter(|finished| finished.ethereal)
                    .count()
                    .try_into()
                    .expect("a fight's plays fit a game number");
                if ethereal != 0 {
                    asked.push(HookEffect::ModifySelf {
                        change: Change::ChangeCost(
                            CostPool::Energy,
                            -ethereal * printed.face(CardVar::Energy),
                        ),
                        lasting: Lasting::ThisFight,
                    });
                }
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
