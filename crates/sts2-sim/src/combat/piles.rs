//! Where a fight's cards stand: five ordered piles over one set of cards.
//!
//! A card is a number, the index its id gives it. The fight mints ids densely
//! from zero, so a card's number is also a place in a table. Each pile is an
//! order over some of those numbers, and every card stands in at most one.
//!
//! [`Piles`] is the interface a fight uses. What stores the order behind it is
//! the implementor's business, and each makes different operations cheap.

/// One of a fight's piles, which is `PlayerCombatState.AllPiles`.
///
/// [`sts2_core::Pile`] and the play pile, which holds the card being played
/// and which no decision names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Place {
    Hand,
    Draw,
    Discard,
    Exhaust,
    Play,
}

impl From<sts2_core::Pile> for Place {
    fn from(pile: sts2_core::Pile) -> Self {
        match pile {
            sts2_core::Pile::Hand => Place::Hand,
            sts2_core::Pile::Draw => Place::Draw,
            sts2_core::Pile::Discard => Place::Discard,
            sts2_core::Pile::Exhaust => Place::Exhaust,
        }
    }
}

/// How many piles there are.
const PLACES: usize = 5;

/// A card's number, which is its id.
pub(crate) type Card = u16;

/// Where a fight's cards stand.
///
/// A pile's front is the end a draw takes from, and its back the end a card
/// arrives at by default. That is the game's own order, which is how every
/// pile is read.
pub(crate) trait Piles: Clone + std::fmt::Debug {
    /// No cards, with room for `cards` of them before anything grows.
    fn with_cards(cards: usize) -> Self;

    fn len(&self, pile: Place) -> usize;

    /// The pile `card` stands in, or nothing for a card no pile holds.
    fn place(&self, card: Card) -> Option<Place>;

    /// Every card in `pile`, front to back.
    fn iter(&self, pile: Place) -> impl ExactSizeIterator<Item = Card> + Clone + '_;

    /// The card at `at` in `pile`.
    fn get(&self, pile: Place, at: usize) -> Option<Card>;

    /// Where `card` stands in `pile`.
    fn position(&self, pile: Place, card: Card) -> Option<usize>;

    /// Take the card at the front of `pile`.
    fn pop_front(&mut self, pile: Place) -> Option<Card>;

    /// Take the card at `at` out of `pile`.
    fn remove_at(&mut self, pile: Place, at: usize) -> Card;

    /// Take `card` out of whichever pile holds it.
    fn remove(&mut self, card: Card);

    /// Put `card` at the back of `pile`.
    fn push_back(&mut self, pile: Place, card: Card);

    /// Put `card` where the card at `at` stood, which leaves no pile.
    fn replace_at(&mut self, pile: Place, at: usize, card: Card);

    /// Move every card of `from` onto the back of `to`, in order.
    fn append(&mut self, to: Place, from: Place);

    /// Swap two piles' cards.
    fn swap(&mut self, a: Place, b: Place);

    /// Reorder `pile` as `arrange` reorders a slice of it, front first.
    fn arrange(&mut self, pile: Place, arrange: impl FnOnce(&mut [Card]));

    /// Keep the cards of `from` that `keep` keeps, in order, and move the
    /// rest onto the back of `to`, in order.
    fn sift(&mut self, from: Place, to: Place, keep: impl FnMut(Card) -> bool);

    /// Every pile emptied.
    fn clear(&mut self);
}

/// Not in a pile.
const NOWHERE: u8 = u8::MAX;

/// Which pile each card stands in, by its number.
#[derive(Debug, Clone, Default)]
struct Places(Vec<u8>);

impl Places {
    fn with_cards(cards: usize) -> Self {
        Places(vec![NOWHERE; cards])
    }

    fn get(&self, card: Card) -> Option<Place> {
        match self.0.get(usize::from(card)).copied() {
            None | Some(NOWHERE) => None,
            Some(at) => Some(PLACE[usize::from(at)]),
        }
    }

    fn set(&mut self, card: Card, place: Option<Place>) {
        let at = usize::from(card);
        if at >= self.0.len() {
            self.0.resize(at + 1, NOWHERE);
        }
        self.0[at] = place.map_or(NOWHERE, |place| place as u8);
    }

    fn clone_from(&mut self, source: &Self) {
        self.0.clone_from(&source.0);
    }
}

const PLACE: [Place; PLACES] = [
    Place::Hand,
    Place::Draw,
    Place::Discard,
    Place::Exhaust,
    Place::Play,
];

/// A `Vec` per pile, front first.
#[derive(Debug)]
pub(crate) struct VecPiles {
    piles: [Vec<Card>; PLACES],
    places: Places,
}

impl Clone for VecPiles {
    fn clone(&self) -> Self {
        Self {
            piles: self.piles.clone(),
            places: self.places.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        for (pile, from) in self.piles.iter_mut().zip(&source.piles) {
            pile.clone_from(from);
        }
        self.places.clone_from(&source.places);
    }
}

impl Piles for VecPiles {
    fn with_cards(cards: usize) -> Self {
        Self {
            piles: std::array::from_fn(|_| Vec::with_capacity(cards)),
            places: Places::with_cards(cards),
        }
    }

    fn len(&self, pile: Place) -> usize {
        self.piles[pile as usize].len()
    }

    fn place(&self, card: Card) -> Option<Place> {
        self.places.get(card)
    }

    fn iter(&self, pile: Place) -> impl ExactSizeIterator<Item = Card> + Clone + '_ {
        self.piles[pile as usize].iter().copied()
    }

    fn get(&self, pile: Place, at: usize) -> Option<Card> {
        self.piles[pile as usize].get(at).copied()
    }

    fn position(&self, pile: Place, card: Card) -> Option<usize> {
        self.piles[pile as usize]
            .iter()
            .position(|&held| held == card)
    }

    fn pop_front(&mut self, pile: Place) -> Option<Card> {
        let cards = &mut self.piles[pile as usize];
        if cards.is_empty() {
            return None;
        }
        let card = cards.remove(0);
        self.places.set(card, None);
        Some(card)
    }

    fn remove_at(&mut self, pile: Place, at: usize) -> Card {
        let card = self.piles[pile as usize].remove(at);
        self.places.set(card, None);
        card
    }

    fn remove(&mut self, card: Card) {
        let Some(pile) = self.places.get(card) else {
            return;
        };
        let at = self
            .position(pile, card)
            .expect("a card stands where its place says");
        self.remove_at(pile, at);
    }

    fn push_back(&mut self, pile: Place, card: Card) {
        self.piles[pile as usize].push(card);
        self.places.set(card, Some(pile));
    }

    fn replace_at(&mut self, pile: Place, at: usize, card: Card) {
        let was = std::mem::replace(&mut self.piles[pile as usize][at], card);
        self.places.set(was, None);
        self.places.set(card, Some(pile));
    }

    fn append(&mut self, to: Place, from: Place) {
        let moved = std::mem::take(&mut self.piles[from as usize]);
        for &card in &moved {
            self.places.set(card, Some(to));
        }
        self.piles[to as usize].extend(moved);
    }

    fn swap(&mut self, a: Place, b: Place) {
        self.piles.swap(a as usize, b as usize);
        for pile in [a, b] {
            for at in 0..self.piles[pile as usize].len() {
                let card = self.piles[pile as usize][at];
                self.places.set(card, Some(pile));
            }
        }
    }

    fn arrange(&mut self, pile: Place, arrange: impl FnOnce(&mut [Card])) {
        arrange(&mut self.piles[pile as usize]);
    }

    fn sift(&mut self, from: Place, to: Place, mut keep: impl FnMut(Card) -> bool) {
        let mut held = std::mem::take(&mut self.piles[from as usize]);
        let mut moved = std::mem::take(&mut self.piles[to as usize]);
        held.retain(|&card| {
            let kept = keep(card);
            if !kept {
                moved.push(card);
                self.places.set(card, Some(to));
            }
            kept
        });
        self.piles[from as usize] = held;
        self.piles[to as usize] = moved;
    }

    fn clear(&mut self) {
        for pile in &mut self.piles {
            pile.clear();
        }
        self.places.0.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cards moved between piles keep their order, and each knows its pile.
    #[test]
    fn piles_keep_order_and_place() {
        let mut piles = VecPiles::with_cards(4);
        for card in 0..6 {
            piles.push_back(Place::Draw, card);
        }
        assert_eq!(piles.pop_front(Place::Draw), Some(0));
        piles.push_back(Place::Hand, 0);
        let taken = piles.remove_at(Place::Draw, 0);
        piles.push_back(Place::Hand, taken);
        piles.remove(3);
        piles.push_back(Place::Discard, 3);
        piles.replace_at(Place::Draw, 0, 7);
        piles.push_back(Place::Exhaust, 2);
        piles.sift(Place::Hand, Place::Discard, |card| card == 1);
        piles.append(Place::Discard, Place::Draw);
        piles.swap(Place::Draw, Place::Discard);
        piles.arrange(Place::Draw, |cards| cards.reverse());

        let pile = |place| piles.iter(place).collect::<Vec<_>>();
        assert_eq!(pile(Place::Hand), [1]);
        assert_eq!(pile(Place::Draw), [5, 4, 7, 0, 3]);
        assert_eq!(pile(Place::Discard), [] as [Card; 0]);
        assert_eq!(pile(Place::Exhaust), [2]);
        assert_eq!(piles.place(7), Some(Place::Draw));
        assert_eq!(piles.place(2), Some(Place::Exhaust));
        assert_eq!(piles.position(Place::Draw, 3), Some(4));
    }
}
