//! A fight's cards: where each stands, and each distinct card state once.
//!
//! A deck can hold a thousand copies of one card. So a card is its id, which
//! the fight mints densely from zero, and an index into [`Cards::faces`]. A
//! rule that writes on one copy stores the state it leaves, and points that
//! copy at it. The copies that shared the old state keep it.
//!
//! A rule that writes on every copy of a card, wherever it is, writes the
//! state itself. Every copy in it changes at once, and no pile is walked.
//!
//! Where each card stands is a [`Piles`], so what stores the order is one
//! type away.

use smallvec::SmallVec;
use sts2_core::rng::GameRng;
use sts2_core::{CardId, CardIdent, CardTable as _, Pile, Rules};

use super::live::{CardInstance, Face};
use super::piles::{Card, Piles, Place};
use crate::rng::MegaRng;

/// What stores the order of a fight's piles.
pub(crate) type PileStore = super::piles::VecPiles;

/// Every card a fight holds, and the five piles they stand in.
#[derive(Debug)]
pub(crate) struct Cards<V: Rules, P: Piles = PileStore> {
    /// Every state a copy has been in this fight. An index into it names one
    /// state for the whole fight. A write on every copy of a card rewrites a
    /// state where it stands, so two indexes can come to hold equal states.
    faces: Vec<Face<V>>,
    /// Each card's index into [`Cards::faces`], by its number.
    states: Vec<u16>,
    piles: P,
}

impl<V: Rules, P: Piles> Clone for Cards<V, P> {
    fn clone(&self) -> Self {
        Self {
            faces: self.faces.clone(),
            states: self.states.clone(),
            piles: self.piles.clone(),
        }
    }

    /// Into the lists already there, so a node a search reuses allocates
    /// nothing.
    fn clone_from(&mut self, source: &Self) {
        self.faces.clone_from(&source.faces);
        self.states.clone_from(&source.states);
        self.piles.clone_from(&source.piles);
    }
}

/// A card's number, which is its id.
fn number<V: Rules>(id: CardId<V>) -> Card {
    num_traits::ToPrimitive::to_u16(&id.get())
        .filter(|&card| card != Card::MAX)
        .unwrap_or_else(|| unimplemented!("a fight holding more cards than a pile numbers"))
}

/// The id a card's number is.
fn id<V: Rules>(card: Card) -> CardId<V> {
    CardId::new(num_traits::NumCast::from(card).expect("a card's number is an id"))
}

impl<V: Rules, P: Piles> Cards<V, P> {
    /// No cards at all, with room for `cards` of them.
    pub(super) fn with_cards(cards: usize) -> Self {
        Self {
            faces: Vec::with_capacity(cards),
            states: Vec::with_capacity(cards),
            piles: P::with_cards(cards),
        }
    }

    /// A fight's opening piles, of fresh copies.
    ///
    /// A fresh copy's state follows from its identity alone. So each identity
    /// is stored once, and its copies point at it. A new identity is a new
    /// state, and is stored without a search.
    ///
    /// An identity is looked up by the card's place in the build's id order,
    /// which no two cards share. The rest of it is compared only on a match.
    pub(super) fn dealt(hand: &[CardInstance<V>], draw: &[CardInstance<V>]) -> Self {
        let mut cards = Self::with_cards(hand.len() + draw.len());
        let mut orders: Vec<u16> = Vec::with_capacity(hand.len() + draw.len());
        for (pile, dealt) in [(Place::Hand, hand), (Place::Draw, draw)] {
            for card in dealt {
                let order = card.face.ident.card.order();
                let found = (0..orders.len())
                    .find(|&at| orders[at] == order && cards.faces[at].ident == card.face.ident);
                let face = match found {
                    Some(at) => at,
                    None => {
                        cards.faces.push(card.face);
                        orders.push(order);
                        cards.faces.len() - 1
                    }
                };
                let face = u16::try_from(face).expect("a deck's identities fit an index");
                cards.set_state(number(card.id), face);
                cards.piles.push_back(pile, number(card.id));
            }
        }
        cards
    }

    pub(crate) fn len(&self, pile: Pile) -> usize {
        self.piles.len(pile.into())
    }

    pub(crate) fn is_empty(&self, pile: Pile) -> bool {
        self.len(pile) == 0
    }

    /// The copy at `at` in `pile`.
    pub(crate) fn get(&self, pile: Pile, at: usize) -> Option<CardInstance<V>> {
        self.piles.get(pile.into(), at).map(|card| self.card(card))
    }

    /// Every copy in `pile`, in pile order.
    pub(crate) fn iter(
        &self,
        pile: Pile,
    ) -> impl ExactSizeIterator<Item = CardInstance<V>> + Clone + '_ {
        self.piles.iter(pile.into()).map(|card| self.card(card))
    }

    /// The identity of every copy in `pile`, in pile order.
    pub(crate) fn idents(&self, pile: Pile) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        self.piles
            .iter(pile.into())
            .map(|card| self.faces[usize::from(self.states[usize::from(card)])].ident)
    }

    /// Where the copy `id` names stands in `pile`.
    pub(crate) fn find(&self, pile: Pile, id: CardId<V>) -> Option<usize> {
        self.piles.position(pile.into(), number(id))
    }

    /// The copy `id` names, wherever the fight holds it.
    pub(crate) fn named(&self, id: CardId<V>) -> Option<CardInstance<V>> {
        let card = number(id);
        self.piles.place(card).map(|_| self.card(card))
    }

    /// Whether any copy outside the exhaust pile is in a state `pick` picks.
    pub(crate) fn any_outside_exhaust(&self, pick: impl Fn(&Face<V>) -> bool) -> bool {
        let picked: SmallVec<[u16; 8]> = (0..self.faces.len())
            .filter(|&at| pick(&self.faces[at]))
            .map(|at| at as u16)
            .collect();
        !picked.is_empty()
            && [Place::Hand, Place::Draw, Place::Discard, Place::Play]
                .into_iter()
                .any(|pile| {
                    self.piles
                        .iter(pile)
                        .any(|card| picked.contains(&self.states[usize::from(card)]))
                })
    }

    /// Add a copy to the end of `pile`.
    pub(crate) fn push(&mut self, pile: Pile, card: CardInstance<V>) {
        self.store(card);
        self.piles.push_back(pile.into(), number(card.id));
    }

    /// Take the copy at `at` out of `pile`.
    pub(crate) fn remove(&mut self, pile: Pile, at: usize) -> CardInstance<V> {
        let card = self.piles.remove_at(pile.into(), at);
        self.card(card)
    }

    /// Move the copy at `at` in `from` to the end of `to`.
    pub(crate) fn move_to(&mut self, from: Pile, at: usize, to: Pile) {
        let card = self.piles.remove_at(from.into(), at);
        self.piles.push_back(to.into(), card);
    }

    /// `CardPileCmd.Draw`'s own step: the card at the front of the draw pile
    /// goes to the hand.
    pub(crate) fn draw(&mut self) -> Option<CardInstance<V>> {
        let card = self.piles.pop_front(Place::Draw)?;
        self.piles.push_back(Place::Hand, card);
        Some(self.card(card))
    }

    /// Put `card` where the copy at `at` stood.
    pub(crate) fn replace(&mut self, pile: Pile, at: usize, card: CardInstance<V>) {
        self.store(card);
        self.piles.replace_at(pile.into(), at, number(card.id));
    }

    /// `AddDuringManualCardPlay`: the card at `at` in hand goes to the play
    /// pile, and stays there while its effect runs.
    pub(crate) fn play(&mut self, at: usize) -> CardInstance<V> {
        let card = self.piles.remove_at(Place::Hand, at);
        self.piles.push_back(Place::Play, card);
        self.card(card)
    }

    /// The card `id` names leaves the play pile, for the end of `to`, or for
    /// nowhere at all.
    ///
    /// # Panics
    ///
    /// A card that is not being played.
    pub(crate) fn leave_play(&mut self, id: CardId<V>, to: Option<Pile>) {
        let card = number(id);
        if self.piles.place(card) != Some(Place::Play) {
            unimplemented!("a card leaving play that was not in play");
        }
        self.piles.remove(card);
        if let Some(to) = to {
            self.piles.push_back(to.into(), card);
        }
    }

    /// Write on the card being played that `id` names.
    ///
    /// # Panics
    ///
    /// A card that is not being played.
    pub(crate) fn write_in_play(&mut self, id: CardId<V>, write: impl FnOnce(&mut Face<V>)) {
        let card = number(id);
        if self.piles.place(card) != Some(Place::Play) {
            unimplemented!("a card written in play that was not in play");
        }
        self.write_card(card, write);
    }

    /// Write on the copy at `at` in `pile`.
    pub(crate) fn write(&mut self, pile: Pile, at: usize, write: impl FnOnce(&mut Face<V>)) {
        let card = self
            .piles
            .get(pile.into(), at)
            .expect("a card stands at the index named");
        self.write_card(card, write);
    }

    fn write_card(&mut self, card: Card, write: impl FnOnce(&mut Face<V>)) {
        let was = self.states[usize::from(card)];
        let mut face = self.faces[usize::from(was)];
        write(&mut face);
        let now = self.restate(was, face);
        self.set_state(card, now);
    }

    /// Write on every copy of a card in the fight that `pick` picks, which is
    /// `PlayerCombatState.AllCards`.
    ///
    /// Each state is written once, where it is stored, so every copy in it
    /// changes together. The cost is the number of states, however many
    /// copies share them.
    pub(crate) fn write_all_cards(
        &mut self,
        pick: impl Fn(&Face<V>) -> bool,
        write: impl Fn(&mut Face<V>),
    ) {
        for face in self.faces.iter_mut().filter(|face| pick(face)) {
            write(face);
        }
    }

    /// Write on every copy in `piles` that `pick` picks, and only those.
    ///
    /// A state can stand in a pile this leaves alone, so each copy written
    /// is pointed at a state of its own. Both are asked once per state.
    pub(crate) fn write_every(
        &mut self,
        piles: &[Pile],
        pick: impl Fn(&Face<V>) -> bool,
        write: impl Fn(&mut Face<V>),
    ) {
        let known = self.faces.len();
        let mut written: SmallVec<[Option<u16>; 64]> = SmallVec::from_elem(None, known);
        for &pile in piles {
            let cards: SmallVec<[Card; 16]> = self.piles.iter(pile.into()).collect();
            for card in cards {
                let was = self.states[usize::from(card)];
                let now = match written[usize::from(was)] {
                    Some(now) => now,
                    None => {
                        let mut face = self.faces[usize::from(was)];
                        let now = match pick(&face) {
                            true => {
                                write(&mut face);
                                self.restate(was, face)
                            }
                            false => was,
                        };
                        written[usize::from(was)] = Some(now);
                        now
                    }
                };
                self.set_state(card, now);
            }
        }
    }

    /// Keep the copies in `from` that `keep` keeps, in order, and move the
    /// rest to the end of `to`, in order.
    pub(crate) fn sift(&mut self, from: Pile, to: Pile, keep: impl Fn(&Face<V>) -> bool) {
        let (faces, states) = (&self.faces, &self.states);
        self.piles.sift(from.into(), to.into(), |card| {
            keep(&faces[usize::from(states[usize::from(card)])])
        });
    }

    /// `CardPileCmd.Shuffle`: the discard pile, then what is left of the draw
    /// pile, stably shuffled into a new draw pile.
    pub(crate) fn reshuffle(&mut self, shuffle: &mut MegaRng) {
        self.piles.append(Place::Discard, Place::Draw);
        self.piles.swap(Place::Draw, Place::Discard);
        // `ModelId.CompareTo`, then the upgrade. A state's key is worked out
        // once here, because the sort asks for it at every comparison.
        let keys: SmallVec<[(u16, u8); 64]> = self
            .faces
            .iter()
            .map(|face| (face.ident.card.order(), face.ident.upgrade))
            .collect();
        let states = &self.states;
        self.piles.arrange(Place::Draw, |cards| {
            shuffle
                .stable_shuffle_by_key(cards, |&card| keys[usize::from(states[usize::from(card)])]);
        });
    }

    /// Every pile emptied, which is how a fight the player lost leaves them.
    pub(crate) fn clear(&mut self) {
        self.piles.clear();
    }

    fn card(&self, card: Card) -> CardInstance<V> {
        self.faces[usize::from(self.states[usize::from(card)])].named(id(card))
    }

    /// Keep `card`'s state against its number.
    fn store(&mut self, card: CardInstance<V>) {
        let at = number(card.id);
        let face = match self.states.get(usize::from(at)) {
            Some(&was) => self.restate(was, card.face),
            None => self.intern(card.face),
        };
        self.set_state(at, face);
    }

    fn set_state(&mut self, card: Card, face: u16) {
        let at = usize::from(card);
        if at >= self.states.len() {
            self.states.resize(at + 1, face);
        }
        self.states[at] = face;
    }

    /// The index a copy in state `was` is in once written to `now`.
    ///
    /// Most writes leave the state as it was, and those search nothing.
    fn restate(&mut self, was: u16, now: Face<V>) -> u16 {
        match self.faces[usize::from(was)] == now {
            true => was,
            false => self.intern(now),
        }
    }

    /// The index `face` is stored at, storing it if nothing is in that state.
    ///
    /// # Panics
    ///
    /// A fight whose copies have been in more states than an index names.
    fn intern(&mut self, face: Face<V>) -> u16 {
        let at = match self.faces.iter().position(|held| *held == face) {
            Some(at) => at,
            None => {
                self.faces.push(face);
                self.faces.len() - 1
            }
        };
        u16::try_from(at)
            .ok()
            .filter(|&at| at != u16::MAX)
            .unwrap_or_else(|| unimplemented!("a fight of {at} card states"))
    }
}
