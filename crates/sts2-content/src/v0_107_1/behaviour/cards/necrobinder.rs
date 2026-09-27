//! The Necrobinder's cards, where the generator leaves them.

use sts2_core::{
    Amount, Change, CostPool, Effect, EventHook, Fight, HookEffect, Lasting, Pile, PileAct, Played,
    Scaling, Selection,
};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardKeyword, CardVar, NecrobinderCard};
use super::super::super::generated::monsters::Monster;
use super::V;

/// `SculptingStrike`: its damage to the target, then one card of the hand
/// that is not Ethereal of its own made Ethereal.
fn sculpting_strike<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(CardVar::Damage),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    });
    let ethereal = CardKeyword::Ethereal;
    let cards = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| !fight.card_keyword(card, ethereal))
        .collect();
    Played::AskingAmong {
        cards,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Necrobinder(Resume::SculptingStrikeEthereal)),
    }
}

pub(super) fn on_play<F: Fight<V>>(card: NecrobinderCard, fight: &mut F) -> Played<V> {
    match card {
        NecrobinderCard::Unleash => osty_attack(fight, CardVar::CalculatedDamage),
        NecrobinderCard::Poke | NecrobinderCard::Flatten => osty_attack(fight, CardVar::OstyDamage),
        NecrobinderCard::Graveblast => graveblast(fight),
        NecrobinderCard::SculptingStrike => sculpting_strike(fight),
        card => card_plays::necrobinder::on_play(card, fight),
    }
}

/// Where a Necrobinder card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Graveblast`, after its owner picked the card to take back.
    GraveblastTakeBack,
    /// `SculptingStrike`, after its owner picked the card to make Ethereal.
    SculptingStrikeEthereal,
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardPileCmd.Add` of the one taken, into hand.
        Resume::GraveblastTakeBack => {
            fight.act_on(PileAct::IntoHand, chosen.as_slice());
            Played::Done
        }
        // `CardCmd.ApplyKeyword(cardModel, CardKeyword.Ethereal)`.
        Resume::SculptingStrikeEthereal => {
            for &card in chosen.as_slice() {
                let ethereal = Change::AddKeyword(CardKeyword::Ethereal);
                fight.modify_card(card, ethereal, Lasting::ThisFight);
            }
            Played::Done
        }
    }
}

/// `Graveblast`: its damage to the target, then one card from the discard
/// pile picked into hand.
fn graveblast<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(CardVar::Damage),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    });
    Played::Asking {
        pile: Pile::Discard,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Necrobinder(Resume::GraveblastTakeBack)),
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
        // alone, and then only once Osty has attacked this turn. A fight here
        // has one player, so any Osty is theirs.
        (NecrobinderCard::Flatten, EventHook::CardEnteredCombat { at, .. }) => {
            if at.id() == id
                && history
                    .attacked_this_turn()
                    .any(|monster| monster == Some(Monster::Osty))
            {
                asked.push(HookEffect::ModifySelf {
                    change: Change::SetCost(CostPool::Energy, 0),
                    lasting: Lasting::ThisTurn,
                });
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
