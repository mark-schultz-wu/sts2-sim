//! The Silent's cards the generator leaves to a hand port.

use sts2_core::{
    Amount, CardIdent, CardKind, Change, CostPool, CreatureView as _, Effect, EventHook, Fight,
    HookEffect, Lasting, Pile, Played, PowerAmount, Rules as _, Scaling, Selection,
};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, Cards, SilentCard, TokenCard};
use super::super::super::generated::powers::Power;
use super::V;

/// Where a Silent card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Survivor`, after its owner picked the card to discard.
    Survivor,
    /// `Prepared`, after its owner picked the cards to discard.
    Prepared,
    /// `DaggerThrow`, after its owner picked the card to discard.
    DaggerThrow,
    /// `HiddenDaggers`, after its owner picked the cards to discard.
    HiddenDaggers,
    /// `Acrobatics`, after its owner picked the card to discard.
    Acrobatics,
}

pub(super) fn on_play<F: Fight<V>>(card: SilentCard, fight: &mut F) -> Played<V> {
    match card {
        SilentCard::Survivor => survivor(fight),
        SilentCard::Prepared => prepared(fight),
        SilentCard::DaggerSpray => dagger_spray(fight),
        SilentCard::LeadingStrike => leading_strike(fight),
        SilentCard::Skewer => skewer(fight),
        SilentCard::DaggerThrow => dagger_throw(fight),
        SilentCard::Haze => haze(fight),
        SilentCard::Malaise => malaise(fight),
        SilentCard::UpMySleeve => up_my_sleeve(fight),
        SilentCard::BladeDance => blade_dance(fight),
        SilentCard::PiercingWail => piercing_wail(fight),
        SilentCard::EchoingSlash => echoing_slash(fight),
        // `Acrobatics`: its `Cards` drawn, then one card of the hand
        // discarded.
        SilentCard::Acrobatics => {
            fight.draw(fight.var(CardVar::Cards));
            Played::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Card(super::Resume::Silent(Resume::Acrobatics)),
            }
        }
        // `Finisher`: its damage to the target, once for each attack its
        // owner finished this turn.
        SilentCard::Finisher => {
            hit_target(fight, fight.var(CardVar::CalculatedHits));
            Played::Done
        }
        // `Expertise`: cards drawn up to its `Cards` in hand.
        SilentCard::Expertise => {
            let hand = super::Number::try_from(fight.cards_in(Pile::Hand).len())
                .expect("a hand fits a game number");
            fight.draw((fight.var(CardVar::Cards) - hand).max(0));
            Played::Done
        }
        // `HiddenDaggers`: its `Cards` of the hand discarded, then Shivs.
        SilentCard::HiddenDaggers => {
            let count = usize::try_from(fight.var(CardVar::Cards))
                .ok()
                .and_then(nonmax::NonMaxUsize::new)
                .expect("a count of cards is not negative");
            Played::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(count),
                at: super::super::Resume::Card(super::Resume::Silent(Resume::HiddenDaggers)),
            }
        }
        // `BouncingFlask`: its Poison on an enemy drawn off `CombatTargets`,
        // its `Repeat` times.
        SilentCard::BouncingFlask => {
            let poison = PowerAmount::Counted(fight.var(CardVar::PoisonPower));
            for _ in 0..fight.var(CardVar::Repeat) {
                if let Some(target) = fight.random_opponent() {
                    fight.apply_power(target, Power::PoisonPower, poison);
                }
            }
            Played::Done
        }
        // `BubbleBubble`: its Poison on the target, where it already carries
        // Poison.
        SilentCard::BubbleBubble => {
            let target = fight.target();
            let poisoned = fight.creatures().any(|creature| {
                creature.id() == target && creature.power(Power::PoisonPower).is_some()
            });
            if poisoned {
                let poison = PowerAmount::Counted(fight.var(CardVar::PoisonPower));
                fight.apply_power(target, Power::PoisonPower, poison);
            }
            Played::Done
        }
        // `Adrenaline`: its energy, then its `Cards` drawn.
        SilentCard::Adrenaline => {
            fight.modify_energy(fight.var(CardVar::Energy));
            fight.draw(fight.var(CardVar::Cards));
            Played::Done
        }
        // `CalculatedGamble`: the whole hand discarded, then as many drawn.
        SilentCard::CalculatedGamble => {
            let hand = fight.cards_in(Pile::Hand);
            let count = super::Number::try_from(hand.len()).expect("a hand fits a game number");
            fight.discard_and_draw(&hand, count);
            Played::Done
        }
        // `Mirage`: its calculated block.
        SilentCard::Mirage => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::CalculatedBlock),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Played::Done
        }
        card => card_plays::silent::on_play(card, fight),
    }
}

/// `Malaise`: its X, one more once upgraded, off the target's Strength, then
/// as Weak on it.
fn malaise<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let amount = fight.x_value() + super::Number::from(fight.upgraded());
    let target = fight.target();
    fight.apply_power(target, Power::StrengthPower, PowerAmount::Counted(-amount));
    fight.apply_power(target, Power::WeakPower, PowerAmount::Counted(amount));
    Played::Done
}

/// `Haze`: its Poison onto each enemy that can be hit, one at a time.
fn haze<F: Fight<V>>(fight: &mut F) -> Played<V> {
    for target in fight.hittable_opponents() {
        fight.apply_power(
            target,
            Power::PoisonPower,
            PowerAmount::Counted(fight.var(CardVar::PoisonPower)),
        );
    }
    Played::Done
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardCmd.Discard` of the one taken.
        Resume::Survivor | Resume::Prepared | Resume::DaggerThrow | Resume::Acrobatics => {
            fight.act_on(sts2_core::PileAct::Discard, chosen.as_slice());
            Played::Done
        }
        // `CardCmd.Discard` of those taken, then its `Shivs` into hand,
        // each upgraded where Hidden Daggers is.
        Resume::HiddenDaggers => {
            fight.act_on(sts2_core::PileAct::Discard, chosen.as_slice());
            let shivs: usize = num_traits::cast(fight.var(CardVar::Shivs))
                .expect("a count of Shivs is not negative");
            let upgraded = fight.upgraded();
            let shiv = CardIdent::new(Cards::Token(TokenCard::Shiv));
            for at in fight.add_generated_cards(shiv, Pile::Hand, shivs) {
                if upgraded {
                    fight.modify_card(at, Change::Upgrade, Lasting::ThisFight);
                }
            }
            Played::Done
        }
    }
}

/// `DaggerThrow`: its damage to the target, one card drawn, then one card
/// from hand to discard.
fn dagger_throw<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    fight.draw(1);
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Silent(Resume::DaggerThrow)),
    }
}

/// Its damage to the target, `hits` times, as one attack.
fn hit_target<F: Fight<V>>(fight: &mut F, hits: super::Number) {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let attack = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(CardVar::Damage),
                scaling: Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    });
}

/// `DaggerSpray`: its damage to every enemy, twice.
fn dagger_spray<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..2 {
            for target in fight.opponents() {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            }
        }
    });
    Played::Done
}

/// `LeadingStrike`: its damage to the target, then its `Shivs` into hand one
/// at a time.
fn leading_strike<F: Fight<V>>(fight: &mut F) -> Played<V> {
    hit_target(fight, 1);
    for _ in 0..fight.var(CardVar::Shivs) {
        fight.add_generated_cards(CardIdent::new(Cards::Token(TokenCard::Shiv)), Pile::Hand, 1);
    }
    Played::Done
}

/// `Skewer`: its damage to the target, once for each energy its X spent.
fn skewer<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let hits = fight.x_value();
    hit_target(fight, hits);
    Played::Done
}

/// `Prepared`: its `Cards` drawn, then as many from hand to discard.
fn prepared<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let cards = fight.var(CardVar::Cards);
    fight.draw(cards);
    let count = usize::try_from(cards)
        .ok()
        .and_then(nonmax::NonMaxUsize::new)
        .unwrap_or_else(|| unimplemented!("Prepared discarding {cards} cards"));
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(count),
        at: super::super::Resume::Card(super::Resume::Silent(Resume::Prepared)),
    }
}

/// `Survivor`: its block, then one card from hand to discard.
fn survivor<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Silent(Resume::Survivor)),
    }
}

/// `UpMySleeve`: its `Cards` of Shivs into hand one at a time, then one off
/// its own cost for the rest of the fight.
fn up_my_sleeve<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let count: usize =
        num_traits::cast(fight.var(CardVar::Cards)).expect("a count of Shivs is not negative");
    for _ in 0..count {
        fight.add_generated_cards(CardIdent::new(Cards::Token(TokenCard::Shiv)), Pile::Hand, 1);
    }
    fight.modify_this(Change::ChangeCost(CostPool::Energy, -1), Lasting::ThisFight);
    Played::Done
}

/// `BladeDance`: its `Cards` of Shivs into hand one at a time.
fn blade_dance<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let count: usize =
        num_traits::cast(fight.var(CardVar::Cards)).expect("a count of Shivs is not negative");
    for _ in 0..count {
        fight.add_generated_cards(CardIdent::new(Cards::Token(TokenCard::Shiv)), Pile::Hand, 1);
    }
    Played::Done
}

/// `PiercingWail`: its `StrengthLoss` as Piercing Wail onto each enemy that
/// can be hit, one at a time.
fn piercing_wail<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let amount = PowerAmount::Counted(fight.var(CardVar::StrengthLoss));
    for target in fight.hittable_opponents() {
        fight.apply_power(target, Power::PiercingWailPower, amount);
    }
    Played::Done
}

/// `EchoingSlash`: its damage to every enemy that can be hit, and again for
/// each one that hit killed, as one attack.
fn echoing_slash<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.attack(Scaling::Dynamic, |fight| {
        let mut left = 1;
        while left > 0 {
            left -= 1;
            for target in fight.hittable_opponents() {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
                // A creature the hit killed has left the fight's list.
                let standing = fight
                    .creatures()
                    .any(|creature| creature.id() == target && creature.alive());
                if !standing {
                    left += 1;
                }
            }
        }
    });
    Played::Done
}

/// Deliver a notification to one Silent card in the fight.
pub(super) fn event_hook(
    card: SilentCard,
    id: sts2_core::CardId<V>,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let skill = |card: Cards| V::card_kind(card) == Some(CardKind::Skill);
    match (card, hook) {
        // `Pinpoint.AfterCardPlayed`: one off its cost this turn for each
        // Skill its owner plays. A fight here has one player.
        (SilentCard::Pinpoint, EventHook::CardPlayed { card, .. }) => {
            if skill(card) {
                asked.push(HookEffect::ModifySelf {
                    change: Change::ChangeCost(CostPool::Energy, -1),
                    lasting: Lasting::ThisTurn,
                });
            }
        }
        // `Pinpoint.AfterCardEnteredCombat`, for its own entry alone and not
        // a clone's: one off this turn for each Skill play finished this turn
        // before it came.
        (SilentCard::Pinpoint, EventHook::CardEnteredCombat { at, clone, .. }) => {
            if at.id() == id && !clone {
                let skills: super::Number = history
                    .finished_this_turn()
                    .filter(|&card| skill(card))
                    .count()
                    .try_into()
                    .expect("a turn's plays fit a game number");
                if skills != 0 {
                    asked.push(HookEffect::ModifySelf {
                        change: Change::ChangeCost(CostPool::Energy, -skills),
                        lasting: Lasting::ThisTurn,
                    });
                }
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
