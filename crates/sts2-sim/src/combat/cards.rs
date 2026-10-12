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

use super::listen::HookCounts;
use super::live::{CardInstance, Face};
use super::piles::{Card, Piles, Place};
use crate::rng::MegaRng;

/// What stores the order of a fight's piles.
pub(crate) type PileStore = super::piles::FlatPiles;

/// Every card a fight holds, and the five piles they stand in.
#[derive(Debug)]
pub(crate) struct Cards<V: Rules, P: Piles = PileStore> {
    /// Every state a copy has been in this fight. An index into it names one
    /// state for the whole fight. A write on every copy of a card rewrites a
    /// state where it stands, so two indexes can come to hold equal states.
    faces: Vec<Face<V>>,
    /// The hooks each state's card overrides, beside [`Cards::faces`]. Which
    /// card a state is never changes, so neither does this.
    heard: Vec<sts2_core::Overrides>,
    /// How many of the copies standing in a pile override each hook. A copy is
    /// counted as it arrives in a pile and uncounted as it leaves them all.
    counts: HookCounts,
    /// Each card's index into [`Cards::faces`], by its number.
    states: Vec<u16>,
    piles: P,
}

impl<V: Rules, P: Piles> Clone for Cards<V, P> {
    fn clone(&self) -> Self {
        Self {
            faces: self.faces.clone(),
            heard: self.heard.clone(),
            counts: self.counts,
            states: self.states.clone(),
            piles: self.piles.clone(),
        }
    }

    /// Into the lists already there, so a node a search reuses allocates
    /// nothing.
    fn clone_from(&mut self, source: &Self) {
        self.faces.clone_from(&source.faces);
        self.heard.clone_from(&source.heard);
        self.counts = source.counts;
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
            heard: Vec::with_capacity(cards),
            counts: HookCounts::default(),
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
        // One for each identity the deck holds, which is inline for a deck of
        // fewer than sixty-four different cards.
        let mut orders: SmallVec<[u16; 64]> = SmallVec::new();
        for (pile, dealt) in [(Place::Hand, hand), (Place::Draw, draw)] {
            for card in dealt {
                let order = card.face.ident.card.order();
                let found = (0..orders.len())
                    .find(|&at| orders[at] == order && cards.faces[at].ident == card.face.ident);
                let face = match found {
                    Some(at) => at,
                    None => {
                        cards.store_face(card.face);
                        orders.push(order);
                        cards.faces.len() - 1
                    }
                };
                let face = u16::try_from(face).expect("a deck's identities fit an index");
                cards.set_state(number(card.id), face);
                cards.piles.push_back(pile, number(card.id));
                cards.counts.add(cards.heard[usize::from(face)]);
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

    /// Whether any card the fight holds overrides a hook in `hooks`.
    pub(crate) fn hears(&self, hooks: sts2_core::Overrides) -> bool {
        self.counts.hears(hooks)
    }

    /// No card the fight holds hears a hook after, as a dead player's
    /// cards hear none.
    pub(crate) fn deafen(&mut self) {
        self.counts.deafen();
    }

    /// Every copy whose card overrides a hook in `hooks`, in `AllPiles`
    /// order: hand, draw, discard, exhaust, then the play pile. Each comes
    /// with every hook its card overrides.
    ///
    /// The states are asked first. An event no card in the fight answers
    /// reads no pile.
    pub(crate) fn listening(
        &self,
        hooks: sts2_core::Overrides,
    ) -> impl Iterator<Item = (CardId<V>, sts2_core::Overrides)> + '_ {
        let hearing: SmallVec<[u16; 8]> = match self.counts.hears(hooks) {
            false => SmallVec::new(),
            true => (0..self.heard.len())
                .filter(|&at| self.heard[at].intersects(hooks))
                .map(|at| at as u16)
                .collect(),
        };
        let piles = match hearing.is_empty() {
            true => [].as_slice(),
            false => [
                Place::Hand,
                Place::Draw,
                Place::Discard,
                Place::Exhaust,
                Place::Play,
            ]
            .as_slice(),
        };
        piles.iter().flat_map(move |&pile| {
            let hearing = hearing.clone();
            self.piles
                .iter(pile)
                .filter(move |&card| hearing.contains(&self.states[usize::from(card)]))
                .map(|card| {
                    (
                        id(card),
                        self.heard[usize::from(self.states[usize::from(card)])],
                    )
                })
        })
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

    /// How many copies in the fight's piles, the exhaust and play piles
    /// included, are in a state `pick` picks. `PlayerCombatState.AllCards`.
    pub(crate) fn count_all(&self, pick: impl Fn(&Face<V>) -> bool) -> usize {
        [
            Place::Hand,
            Place::Draw,
            Place::Discard,
            Place::Exhaust,
            Place::Play,
        ]
        .into_iter()
        .map(|pile| {
            self.piles
                .iter(pile)
                .filter(|&card| pick(&self.faces[usize::from(self.states[usize::from(card)])]))
                .count()
        })
        .sum()
    }

    /// Add a copy to the end of `pile`.
    pub(crate) fn push(&mut self, pile: Pile, card: CardInstance<V>) {
        self.store(card);
        self.piles.push_back(pile.into(), number(card.id));
        self.counts.add(self.overrides(number(card.id)));
    }

    /// `CardPileCmd.Add` to the play pile of a copy no pile held, which an
    /// auto-play makes of a dupe.
    pub(crate) fn enter_play(&mut self, card: CardInstance<V>) {
        self.store(card);
        self.piles.push_back(Place::Play, number(card.id));
        self.counts.add(self.overrides(number(card.id)));
    }

    /// Put `card` into `pile` at `at`, counting from the front.
    pub(crate) fn insert(&mut self, pile: Pile, at: usize, card: CardInstance<V>) {
        self.push(pile, card);
        self.piles
            .arrange(pile.into(), |cards| cards[at..].rotate_right(1));
    }

    /// Take the copy at `at` out of `pile`.
    pub(crate) fn remove(&mut self, pile: Pile, at: usize) -> CardInstance<V> {
        let card = self.piles.remove_at(pile.into(), at);
        self.counts.remove(self.overrides(card));
        self.card(card)
    }

    /// Move the copy at `at` in `from` to the end of `to`.
    pub(crate) fn move_to(&mut self, from: Pile, at: usize, to: Pile) {
        self.piles.move_to(from.into(), at, to.into());
    }

    /// `CardPileCmd.Draw`'s own step: the card at the front of the draw pile
    /// goes to the hand.
    pub(crate) fn draw(&mut self) -> Option<CardInstance<V>> {
        if self.piles.len(Place::Draw) == 0 {
            return None;
        }
        let card = self.piles.move_to(Place::Draw, 0, Place::Hand);
        Some(self.card(card))
    }

    /// Put `card` where the copy at `at` stood.
    pub(crate) fn replace(&mut self, pile: Pile, at: usize, card: CardInstance<V>) {
        let was = self
            .piles
            .get(pile.into(), at)
            .expect("a card stands at the index named");
        self.counts.remove(self.overrides(was));
        self.store(card);
        self.piles.replace_at(pile.into(), at, number(card.id));
        self.counts.add(self.overrides(number(card.id)));
    }

    /// `AddDuringManualCardPlay`, or an auto-play's `CardPileCmd.Add`: the
    /// card at `at` in `from` goes to the play pile, and stays there while its
    /// effect runs.
    pub(crate) fn play(&mut self, from: Pile, at: usize) -> CardInstance<V> {
        let card = self.piles.move_to(from.into(), at, Place::Play);
        self.card(card)
    }

    /// The card `id` names leaves the play pile, for the end of `to`, or for
    /// nowhere at all.
    ///
    /// # Panics
    ///
    /// A card that is not being played.
    pub(crate) fn leave_play(
        &mut self,
        id: CardId<V>,
        to: Option<Pile>,
        position: sts2_core::PilePosition,
    ) {
        let card = number(id);
        if self.piles.place(card) != Some(Place::Play) {
            unimplemented!("a card leaving play that was not in play");
        }
        let at = self
            .piles
            .position(Place::Play, card)
            .expect("a card stands where its place says");
        match to {
            Some(to) => {
                self.piles.move_to(Place::Play, at, to.into());
                // `CardPilePosition.Top` is the front, where a draw takes from.
                if position == sts2_core::PilePosition::Top {
                    self.piles.arrange(to.into(), |cards| cards.rotate_right(1));
                }
            }
            None => {
                self.piles.remove_at(Place::Play, at);
                self.counts.remove(self.overrides(card));
            }
        }
    }

    /// `CardPileCmd.Add` of the card `id` names, from the pile outside play
    /// it stands in, for `to` at `position`. A card already in `to` moves
    /// within it.
    ///
    /// # Panics
    ///
    /// A card in play, which [`Self::leave_play`] moves, or one the fight
    /// does not hold.
    pub(crate) fn move_named(
        &mut self,
        id: CardId<V>,
        to: Pile,
        position: sts2_core::PilePosition,
    ) {
        let card = number(id);
        let from = match self.piles.place(card) {
            Some(Place::Play) => unimplemented!("moving {id:?} out of play outside its play"),
            Some(from) => from,
            None => unimplemented!("moving {id:?}, which the fight does not hold"),
        };
        let at = self
            .piles
            .position(from, card)
            .expect("a card stands where its place says");
        self.piles.move_to(from, at, to.into());
        // `CardPilePosition.Top` is the front, where a draw takes from.
        if position == sts2_core::PilePosition::Top {
            self.piles.arrange(to.into(), |cards| cards.rotate_right(1));
        }
    }

    /// The pile the copy `id` stands in, and nothing for the play pile or a
    /// copy the fight does not hold.
    pub(crate) fn pile_of(&self, id: CardId<V>) -> Option<Pile> {
        match self.piles.place(number(id))? {
            Place::Hand => Some(Pile::Hand),
            Place::Draw => Some(Pile::Draw),
            Place::Discard => Some(Pile::Discard),
            Place::Exhaust => Some(Pile::Exhaust),
            Place::Play => None,
        }
    }

    /// Whether the copy `id` stands in the play pile.
    pub(crate) fn in_play(&self, id: CardId<V>) -> bool {
        self.piles.place(number(id)) == Some(Place::Play)
    }

    /// Write on the card being played that `id` names.
    ///
    /// # Panics
    ///
    /// A card that is not being played.
    pub(crate) fn write_in_play(
        &mut self,
        id: CardId<V>,
        write: impl FnOnce(&mut Face<V>) -> bool,
    ) {
        let card = number(id);
        if self.piles.place(card) != Some(Place::Play) {
            unimplemented!("a card written in play that was not in play");
        }
        self.write_card(card, write);
    }

    /// Write on the copy `id` names, wherever the fight holds it.
    ///
    /// # Panics
    ///
    /// A copy the fight does not hold.
    pub(crate) fn write_named(&mut self, id: CardId<V>, write: impl FnOnce(&mut Face<V>) -> bool) {
        let card = number(id);
        if self.piles.place(card).is_none() {
            unimplemented!("a card written after it left the fight");
        }
        self.write_card(card, write);
    }

    /// Write on the copy at `at` in `pile`.
    pub(crate) fn write(
        &mut self,
        pile: Pile,
        at: usize,
        write: impl FnOnce(&mut Face<V>) -> bool,
    ) {
        let card = self
            .piles
            .get(pile.into(), at)
            .expect("a card stands at the index named");
        self.write_card(card, write);
    }

    /// Write on the copy numbered `card`, and keep the state it ends in.
    ///
    /// `write` says whether it changed anything. A write that did not leaves
    /// the copy in the state it was in, with nothing to compare.
    fn write_card(&mut self, card: Card, write: impl FnOnce(&mut Face<V>) -> bool) {
        let was = self.states[usize::from(card)];
        let mut face = self.faces[usize::from(was)];
        if !write(&mut face) {
            return;
        }
        let now = self.restate(was, face);
        // A write that changes what the copy is, such as an enchantment put
        // on it, changes what it hears.
        if self.heard[usize::from(was)] != self.heard[usize::from(now)] {
            self.counts.remove(self.heard[usize::from(was)]);
            self.counts.add(self.heard[usize::from(now)]);
        }
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

    /// `SetupPlayerTurn` on turn one. Each copy `sinks` names goes to the draw
    /// pile's bottom in pile order, so they keep their order there. Then each
    /// other innate copy goes to its top in pile order, so the last of them
    /// ends up first. How many innate copies went up.
    pub(crate) fn innate_to_top(&mut self, sinks: impl Fn(&Face<V>) -> bool) -> usize {
        let (faces, states) = (&self.faces, &self.states);
        let face = |card: Card| &faces[usize::from(states[usize::from(card)])];
        let mut innate = 0;
        self.piles.arrange(Place::Draw, |cards| {
            let mut sunk: SmallVec<[Card; 4]> = SmallVec::new();
            let mut kept = 0;
            for at in 0..cards.len() {
                let card = cards[at];
                if sinks(face(card)) {
                    sunk.push(card);
                } else {
                    cards[kept] = card;
                    kept += 1;
                }
            }
            cards[kept..].copy_from_slice(&sunk);
            for at in 0..kept {
                if face(cards[at]).innate() {
                    cards[..=at].rotate_right(1);
                    innate += 1;
                }
            }
        });
        innate
    }

    /// The copy at `at` in `pile` moved to its top, the rest keeping their
    /// order behind it.
    pub(crate) fn move_to_top(&mut self, pile: Pile, at: usize) {
        self.piles
            .arrange(pile.into(), |cards| cards[..=at].rotate_right(1));
    }

    /// Every pile emptied, which is how a fight the player lost leaves them.
    pub(crate) fn clear(&mut self) {
        self.piles.clear();
        self.counts = HookCounts::default();
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

    /// Store a new state, with the hooks its card overrides.
    fn store_face(&mut self, face: Face<V>) {
        self.faces.push(face);
        // A card hears what its enchantment's class overrides as well.
        let enchantment = face
            .ident
            .enchantment
            .map_or_else(sts2_core::Overrides::default, |on| {
                <V::Enchantment as sts2_core::EnchantmentTable>::overrides(on.enchantment)
            });
        self.heard
            .push(face.ident.card.overrides().union(enchantment));
    }

    /// The hooks the card at number `card` overrides.
    fn overrides(&self, card: Card) -> sts2_core::Overrides {
        self.heard[usize::from(self.states[usize::from(card)])]
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
                self.store_face(face);
                self.faces.len() - 1
            }
        };
        u16::try_from(at)
            .ok()
            .filter(|&at| at != u16::MAX)
            .unwrap_or_else(|| unimplemented!("a fight of {at} card states"))
    }
}

#[cfg(test)]
mod tests {
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_content::v0_107_1::cards::{Cards as Card, RegentCard};
    use sts2_core::CardIdent;

    use super::super::live::{CardIds, CardInstance};
    use super::{Cards, Pile};

    /// The fight's first draw puts the innate cards on top.
    ///
    /// Two of them, so the order they end up in is visible. Each is moved to
    /// the top in turn, which leaves the later one in front.
    #[test]
    fn the_innate_cards_go_on_top_the_later_first() {
        let mut ids = CardIds::<V0_107_1>::new();
        let plain = CardIdent::new(Card::Regent(RegentCard::Strike));
        // `BigBang.OnUpgrade` is `AddKeyword(Innate)`, so an upgraded one
        // opens the fight in hand.
        let innate = CardIdent {
            upgrade: 1,
            ..CardIdent::new(Card::Regent(RegentCard::BigBang))
        };
        let deck = [plain, innate, plain, plain, plain, innate, plain];
        let draw: Vec<_> = deck
            .iter()
            .map(|card| CardInstance::new(*card, ids.mint()))
            .collect();
        let named: Vec<_> = draw.iter().map(|card| card.id).collect();
        let mut cards = Cards::<V0_107_1>::dealt(&[], &draw);

        assert_eq!(cards.innate_to_top(|_| false), 2);
        let top: Vec<_> = (0..2)
            .map(|at| cards.get(Pile::Draw, at).expect("a card").id)
            .collect();
        assert_eq!(top, [named[5], named[1]]);
    }
}
