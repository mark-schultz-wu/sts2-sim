//! The colorless cards the generator leaves to a hand port.

use sts2_core::{
    CardKind, EventHook, Fight, HookEffect, Pile, PileAct, Played, Rules as _, Selection,
};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardRarity, CardVar, ColorlessCard};
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: ColorlessCard, fight: &mut F) -> Played<V> {
    match card {
        ColorlessCard::Restlessness => restlessness(fight),
        ColorlessCard::Impatience => impatience(fight),
        ColorlessCard::Anointed => anointed(fight),
        // `FromHand` for up to its `Cards`, each then exhausted.
        ColorlessCard::Purity => match nonmax::NonMaxUsize::new(cards(fight)) {
            Some(count) => Played::Asking {
                pile: Pile::Hand,
                selection: Selection::UpTo(count),
                at: super::super::Resume::Card(super::Resume::Colorless(Resume::PurityExhausts)),
            },
            None => Played::Done,
        },
        card => card_plays::colorless::on_play(card, fight),
    }
}

/// Where a colorless card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Purity`, after the cards picked from hand to exhaust.
    PurityExhausts,
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        Resume::PurityExhausts => {
            fight.act_on(PileAct::Exhaust, chosen.as_slice());
            Played::Done
        }
    }
}

/// The card's `Cards`, as a count.
fn cards<F: Fight<V>>(fight: &F) -> usize {
    num_traits::cast(fight.var(CardVar::Cards)).expect("a count of cards is not negative")
}

/// `Impatience`: its `Cards` drawn, with no attack in hand.
fn impatience<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let attack = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .any(|at| V::card_kind(fight.card_ident(at).card) == Some(CardKind::Attack));
    if !attack {
        fight.draw(fight.var(CardVar::Cards));
    }
    Played::Done
}

/// `Anointed`: the rare cards in the draw pile, `TakeRandom` off
/// `CombatCardSelection` for as many as the hand has room for, into hand.
fn anointed<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CardTable as _;

    let room = sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
    let mut rares: Vec<_> = fight
        .cards_in(Pile::Draw)
        .into_iter()
        .filter(|&at| fight.card_ident(at).card.rarity() == CardRarity::Rare)
        .collect();
    let taken = fight.take_random_cards(&mut rares, room);
    fight.act_on(PileAct::IntoHand, &taken);
    Played::Done
}

/// `Restlessness`: with no other card in hand, its `Cards` drawn one at a
/// time, then its `Energy`. It stands in the play pile as it runs.
fn restlessness<F: Fight<V>>(fight: &mut F) -> Played<V> {
    if fight.cards_in(Pile::Hand).is_empty() {
        for _ in 0..fight.var(CardVar::Cards) {
            fight.draw(1);
        }
        fight.modify_energy(fight.var(CardVar::Energy));
    }
    Played::Done
}

/// Deliver a notification to one colorless card in the fight.
pub(super) fn event_hook(
    card: ColorlessCard,
    id: sts2_core::CardId<V>,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (card, hook) {
        // `Bolas.BeforeHandDraw` and `ThrummingHatchet.BeforeHandDraw`: back
        // into hand, from wherever it is, once this copy finished a play in
        // its owner's last turn. A fight here has one player.
        (
            ColorlessCard::Bolas | ColorlessCard::ThrummingHatchet,
            EventHook::BeforeHandDraw { turn, .. },
        ) => {
            if history
                .finished_last_player_turn(turn)
                .any(|finished| finished.id == id)
            {
                asked.push(HookEffect::ToHand);
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
