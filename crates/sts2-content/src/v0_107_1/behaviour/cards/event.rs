//! The event cards the generator leaves to a hand port.

use sts2_core::{
    Amount, Change, CostPool, Effect, Fight, Lasting, Pile, PileAct, Played, Scaling, Selection,
};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardType, CardVar, EventCard};
use super::V;

/// Where an event card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `NeowsFury`, after its owner picked the cards to take back.
    NeowsFuryPicks,
}

pub(super) fn on_play<F: Fight<V>>(card: EventCard, fight: &mut F) -> Played<V> {
    match card {
        EventCard::NeowsFury => neows_fury(fight),
        EventCard::Metamorphosis => metamorphosis(fight),
        // `BrightestFlame`: its Energy, its Cards drawn, then its `MaxHp` off
        // its owner's maximum.
        EventCard::BrightestFlame => {
            fight.modify_energy(fight.var(CardVar::Energy));
            fight.draw(fight.var(CardVar::Cards));
            fight.lose_max_hp(fight.actor(), fight.var(CardVar::MaxHp));
            Played::Done
        }
        // Its damage to the target. The Byrdpip pet only animates the hit.
        EventCard::ByrdSwoop => {
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
            Played::Done
        }
        card => card_plays::event::on_play(card, fight),
    }
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardPileCmd.Add(picked, PileType.Hand)`.
        Resume::NeowsFuryPicks => {
            fight.act_on(PileAct::IntoHand, chosen.as_slice());
            Played::Done
        }
    }
}

/// `NeowsFury`: its damage to the target, then up to its `Cards` from the
/// discard pile into hand, as many as the hand has room for.
fn neows_fury<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    let cards: usize =
        num_traits::cast(fight.var(CardVar::Cards)).expect("a count of cards is not negative");
    let room = sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
    match nonmax::NonMaxUsize::new(cards.min(room)) {
        Some(count) if count.get() > 0 => Played::Asking {
            pile: Pile::Discard,
            selection: Selection::UpTo(count),
            at: super::super::Resume::Card(super::Resume::Event(Resume::NeowsFuryPicks)),
        },
        _ => Played::Done,
    }
}

/// `Metamorphosis`: its `Cards` attacks from the character's pool, each
/// `NextItem` off `CombatCardGeneration`, free this fight and into the draw
/// pile at a random place.
fn metamorphosis<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let attacks = super::super::character_cards(fight, CardType::Attack);
    for _ in 0..fight.var(CardVar::Cards) {
        let Some(card) = fight.generate_card(&attacks) else {
            unimplemented!("a character pool with no attack")
        };
        if let Some(at) = fight.add_at_random(card, Pile::Draw) {
            for pool in [CostPool::Energy, CostPool::Star] {
                fight.modify_card(
                    at,
                    Change::SetCost(pool, num_traits::zero()),
                    Lasting::ThisFight,
                );
            }
        }
    }
    Played::Done
}
