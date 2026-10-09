//! Where a fight's cards stand: five ordered piles over one set of cards.
//!
//! A card is a number, the index its id gives it. The fight mints ids densely
//! from zero, so a card's number is also a place in a table. Each pile is an
//! order over some of those numbers, and every card stands in at most one.
//!
//! [`Piles`] is the interface a fight uses. What stores the order behind it is
//! the implementor's business, and each makes different operations cheap.

/// One of a fight's piles, which is `PlayerCombatState.AllPiles`: the
/// storage's own name for a [`sts2_core::Pile`].
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
            sts2_core::Pile::Play => Place::Play,
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

    /// Take the card at `at` out of `pile`.
    fn remove_at(&mut self, pile: Place, at: usize) -> Card;

    /// Put `card` at the back of `pile`.
    fn push_back(&mut self, pile: Place, card: Card);

    /// Move the card at `at` in `from` to the back of `to`, and say which it
    /// was.
    fn move_to(&mut self, from: Place, at: usize, to: Place) -> Card;

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
#[cfg(test)]
const NOWHERE: u8 = u8::MAX;

/// Which pile each card stands in, by its number.
#[cfg(test)]
#[derive(Debug, Clone, Default)]
struct Places(Vec<u8>);

#[cfg(test)]
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

/// A `Vec` per pile, front first: the reference [`FlatPiles`] is held to.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct VecPiles {
    piles: [Vec<Card>; PLACES],
    places: Places,
}

#[cfg(test)]
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

#[cfg(test)]
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

    fn remove_at(&mut self, pile: Place, at: usize) -> Card {
        let card = self.piles[pile as usize].remove(at);
        self.places.set(card, None);
        card
    }

    fn push_back(&mut self, pile: Place, card: Card) {
        self.piles[pile as usize].push(card);
        self.places.set(card, Some(pile));
    }

    fn move_to(&mut self, from: Place, at: usize, to: Place) -> Card {
        let card = self.remove_at(from, at);
        self.push_back(to, card);
        card
    }

    fn replace_at(&mut self, pile: Place, at: usize, card: Card) {
        let was = std::mem::replace(&mut self.piles[pile as usize][at], card);
        self.places.set(was, None);
        self.places.set(card, Some(pile));
    }

    fn append(&mut self, to: Place, from: Place) {
        let mut moved = std::mem::take(&mut self.piles[from as usize]);
        for &card in &moved {
            self.places.set(card, Some(to));
        }
        self.piles[to as usize].extend_from_slice(&moved);
        // The emptied pile keeps its room, as every pile holds a whole deck's.
        moved.clear();
        self.piles[from as usize] = moved;
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

/// Every pile in one buffer, each in a region of its own.
///
/// Six regions of one width: each card's place, by its number, then one
/// region a pile, front first, in [`Place`] order. No pile holds more cards
/// than there are numbers, so none outgrows its region. The buffer grows
/// only when a card's number passes the width.
///
/// A card moves between two piles as it does between two lists.
#[derive(Debug)]
pub(crate) struct FlatPiles {
    buf: Vec<Card>,
    /// How many cards each region holds room for.
    width: usize,
    /// How many cards each pile holds, by [`Place`].
    lens: [u16; PLACES],
}

/// Not in a pile, as the places region spells it.
const NO_PLACE: Card = Card::MAX;

impl Clone for FlatPiles {
    fn clone(&self) -> Self {
        Self {
            buf: self.buf.clone(),
            width: self.width,
            lens: self.lens,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.buf.clone_from(&source.buf);
        self.width = source.width;
        self.lens = source.lens;
    }
}

impl FlatPiles {
    /// Where `pile`'s region starts.
    fn base(&self, pile: Place) -> usize {
        self.width * (1 + pile as usize)
    }

    fn pile(&self, pile: Place) -> &[Card] {
        let base = self.base(pile);
        &self.buf[base..base + self.len(pile)]
    }

    fn pile_mut(&mut self, pile: Place) -> &mut [Card] {
        let (base, len) = (self.base(pile), self.len(pile));
        &mut self.buf[base..base + len]
    }

    /// Room for a card numbered `card`, which is asked as a card arrives.
    #[inline]
    fn fit(&mut self, card: Card) {
        if usize::from(card) >= self.width {
            self.grow(card);
        }
    }

    #[cold]
    fn grow(&mut self, card: Card) {
        let width = (usize::from(card) + 1).max(self.width * 2);
        let mut buf = vec![NO_PLACE; (1 + PLACES) * width];
        buf[..self.width].copy_from_slice(&self.buf[..self.width]);
        for pile in PLACE {
            let at = width * (1 + pile as usize);
            buf[at..at + self.len(pile)].copy_from_slice(self.pile(pile));
        }
        self.buf = buf;
        self.width = width;
    }

    /// Where `card` stands, for a card [`FlatPiles::fit`] has made room for.
    fn set_place(&mut self, card: Card, place: Option<Place>) {
        self.buf[usize::from(card)] = place.map_or(NO_PLACE, |place| place as Card);
    }
}

impl Piles for FlatPiles {
    fn with_cards(cards: usize) -> Self {
        Self {
            buf: vec![NO_PLACE; (1 + PLACES) * cards],
            width: cards,
            lens: [0; PLACES],
        }
    }

    fn len(&self, pile: Place) -> usize {
        usize::from(self.lens[pile as usize])
    }

    fn place(&self, card: Card) -> Option<Place> {
        match self.buf[..self.width].get(usize::from(card)).copied() {
            None | Some(NO_PLACE) => None,
            Some(at) => Some(PLACE[usize::from(at)]),
        }
    }

    fn iter(&self, pile: Place) -> impl ExactSizeIterator<Item = Card> + Clone + '_ {
        self.pile(pile).iter().copied()
    }

    fn get(&self, pile: Place, at: usize) -> Option<Card> {
        self.pile(pile).get(at).copied()
    }

    fn position(&self, pile: Place, card: Card) -> Option<usize> {
        self.pile(pile).iter().position(|&held| held == card)
    }

    fn remove_at(&mut self, pile: Place, at: usize) -> Card {
        let cards = self.pile_mut(pile);
        let card = cards[at];
        cards.copy_within(at + 1.., at);
        self.lens[pile as usize] -= 1;
        self.set_place(card, None);
        card
    }

    fn push_back(&mut self, pile: Place, card: Card) {
        self.fit(card);
        let at = self.base(pile) + self.len(pile);
        self.buf[at] = card;
        self.lens[pile as usize] += 1;
        self.set_place(card, Some(pile));
    }

    fn move_to(&mut self, from: Place, at: usize, to: Place) -> Card {
        let card = self.remove_at(from, at);
        let back = self.base(to) + self.len(to);
        self.buf[back] = card;
        self.lens[to as usize] += 1;
        self.set_place(card, Some(to));
        card
    }

    fn replace_at(&mut self, pile: Place, at: usize, card: Card) {
        self.fit(card);
        let at = self.base(pile) + at;
        let was = std::mem::replace(&mut self.buf[at], card);
        self.set_place(was, None);
        self.set_place(card, Some(pile));
    }

    fn append(&mut self, to: Place, from: Place) {
        let (source, count) = (self.base(from), self.len(from));
        let back = self.base(to) + self.len(to);
        self.buf.copy_within(source..source + count, back);
        for at in back..back + count {
            let card = self.buf[at];
            self.set_place(card, Some(to));
        }
        self.lens[to as usize] += self.lens[from as usize];
        self.lens[from as usize] = 0;
    }

    fn swap(&mut self, a: Place, b: Place) {
        if a == b {
            return;
        }
        let (low, high) = match a as usize <= b as usize {
            true => (a, b),
            false => (b, a),
        };
        let count = self.len(a).max(self.len(b));
        let (base, other) = (self.base(low), self.base(high));
        let (front, back) = self.buf.split_at_mut(other);
        front[base..base + count].swap_with_slice(&mut back[..count]);
        self.lens.swap(a as usize, b as usize);
        for pile in [a, b] {
            for at in 0..self.len(pile) {
                let card = self.pile(pile)[at];
                self.set_place(card, Some(pile));
            }
        }
    }

    fn arrange(&mut self, pile: Place, arrange: impl FnOnce(&mut [Card])) {
        arrange(self.pile_mut(pile));
    }

    fn sift(&mut self, from: Place, to: Place, mut keep: impl FnMut(Card) -> bool) {
        let source = self.base(from);
        let mut kept = source;
        let mut back = self.base(to) + self.len(to);
        for at in source..source + self.len(from) {
            let card = self.buf[at];
            if keep(card) {
                self.buf[kept] = card;
                kept += 1;
            } else {
                self.buf[back] = card;
                back += 1;
                self.set_place(card, Some(to));
            }
        }
        let moved = self.len(from) - (kept - source);
        self.lens[from as usize] -= u16::try_from(moved).expect("a pile's cards fit its length");
        self.lens[to as usize] += u16::try_from(moved).expect("a pile's cards fit its length");
    }

    fn clear(&mut self) {
        self.buf.fill(NO_PLACE);
        self.lens = [0; PLACES];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cards moved between piles keep their order, and each knows its pile.
    #[test]
    fn piles_keep_order_and_place() {
        keep_order_and_place::<VecPiles>();
        keep_order_and_place::<FlatPiles>();
    }

    fn keep_order_and_place<P: Piles>() {
        let mut piles = P::with_cards(4);
        for card in 0..6 {
            piles.push_back(Place::Draw, card);
        }
        assert_eq!(piles.remove_at(Place::Draw, 0), 0);
        piles.push_back(Place::Hand, 0);
        let taken = piles.remove_at(Place::Draw, 0);
        piles.push_back(Place::Hand, taken);
        let at = piles
            .position(Place::Draw, 3)
            .expect("card 3 is in the draw pile");
        piles.move_to(Place::Draw, at, Place::Discard);
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

    /// The one buffer holds the same piles as a `Vec` each, through any run
    /// of moves. Numbers past the first width make it grow.
    #[test]
    fn flat_piles_follow_vec_piles() {
        const CARDS: Card = 40;
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = |below: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % below as u64) as usize
        };
        let mut reference = VecPiles::with_cards(8);
        let mut flat = FlatPiles::with_cards(8);
        for step in 0..20_000 {
            let pile = PLACE[next(PLACES)];
            let other = PLACE[next(PLACES)];
            let loose: Vec<Card> = (0..CARDS)
                .filter(|&card| reference.place(card).is_none())
                .collect();
            match next(12) {
                0..=2 if !loose.is_empty() => {
                    let card = loose[next(loose.len())];
                    reference.push_back(pile, card);
                    flat.push_back(pile, card);
                }
                10..=11 if reference.len(pile) > 0 => {
                    let at = next(reference.len(pile));
                    assert_eq!(
                        reference.move_to(pile, at, other),
                        flat.move_to(pile, at, other)
                    );
                }
                4 if reference.len(pile) > 0 => {
                    let at = next(reference.len(pile));
                    assert_eq!(reference.remove_at(pile, at), flat.remove_at(pile, at));
                }
                6 if reference.len(pile) > 0 && !loose.is_empty() => {
                    let (at, card) = (next(reference.len(pile)), loose[next(loose.len())]);
                    reference.replace_at(pile, at, card);
                    flat.replace_at(pile, at, card);
                }
                7 if pile != other => {
                    reference.append(other, pile);
                    flat.append(other, pile);
                }
                8 if pile != other => {
                    reference.swap(pile, other);
                    flat.swap(pile, other);
                }
                9 if pile != other => {
                    let modulus = next(3) as Card + 2;
                    reference.sift(pile, other, |card| card % modulus != 0);
                    flat.sift(pile, other, |card| card % modulus != 0);
                }
                _ => {
                    let by = next(4);
                    let turn = |cards: &mut [Card]| {
                        if !cards.is_empty() {
                            cards.rotate_left(by % cards.len());
                        }
                    };
                    reference.arrange(pile, turn);
                    flat.arrange(pile, turn);
                }
            }
            for place in PLACE {
                assert!(
                    reference.iter(place).eq(flat.iter(place)),
                    "{place:?} after step {step}"
                );
            }
            for card in 0..CARDS {
                assert_eq!(
                    reference.place(card),
                    flat.place(card),
                    "card {card} at {step}"
                );
            }
            if step % 5_000 == 4_999 {
                reference.clear();
                flat.clear();
            }
        }
    }
}
