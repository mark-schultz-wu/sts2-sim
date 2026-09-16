//! Three things the game names with one string each.
//!
//! A map coordinate, a card's identity, and a model's id. Each is more than one
//! field, and the oracle packs each into a single string. So each needs a type
//! on this side to be unpacked into.
//!
//! No spelling here. `"3,11"` and `"MONSTER/BYRDPIP"` are the oracle's choices,
//! and live in `sts2-headless` beside the rest of its wire. That is the same
//! reason `Decision` carries no `serde`. A second implementation can spell a
//! coordinate any way it likes and still be talking about this coordinate.

use crate::ContentVersion;

/// The largest byte, which the game's small numbers never reach.
///
/// [`CombatId`](crate::CombatId), [`HandIndex`](crate::HandIndex) and
/// [`BeltIndex`](crate::BeltIndex) leave it free, so that an `Option` of one is
/// still a byte wide. Each refuses it, and this is what each says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the largest byte names nothing")]
pub struct OutOfRange;

/// Where a node sits on an act's map.
///
/// Both are `u8` because the game says so. `MapCoord.Serialize` writes each as
/// one byte into a multiplayer packet.
///
/// Ordered column-major, which is `MapCoord.CompareTo`. Not the order the
/// observation lists points in, which is a presentation order belonging to the
/// list rather than to a coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Coord {
    pub col: u8,
    pub row: u8,
}

impl Coord {
    pub const fn new(col: u8, row: u8) -> Coord {
        Coord { col, row }
    }
}

impl std::fmt::Display for Coord {
    /// `col,row`, the observation's spelling. A `Coord` in a message to a
    /// person reads the way the same coordinate reads in the state beside it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{}", self.col, self.row)
    }
}

/// Which card this is, to the precision that comparing two decks needs.
///
/// The id alone is not enough. Two decks differing only by an upgrade are
/// different decks under the same id, so everything a card carries into a pile
/// travels with it.
///
/// The three are not a judgement about what matters. They are what
/// `CombatState.IterateHookListeners` walks off a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CardIdent<V: ContentVersion> {
    pub card: V::Card,
    /// How many times it has been upgraded. Zero for a card that has not been.
    ///
    /// A count and not a flag. `CurrentUpgradeLevel` is an integer and the
    /// oracle writes it out, so a card upgraded twice is not a card upgraded
    /// once.
    pub upgrade: u8,
    /// The affliction stuck to this card, for the cards that have one.
    pub affliction: Option<V::Affliction>,
    /// The enchantment on this card, for the cards that have one.
    ///
    /// Independent of the affliction. A card can carry both, and the game keeps
    /// them in two fields. So this is a second option rather than one sum over
    /// the two.
    pub enchantment: Option<V::Enchantment>,
}

/// Which card instance a fight is talking about.
///
/// [`CardIdent`] says what a card is. This says which one. A fight moves its
/// copies between piles, so a position names a card only until the next thing
/// that touches one.
///
/// Minted per fight and never reused. A card transformed into another is a new
/// card with a new id, as the game creates a new model for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CardId<V: ContentVersion>(V::CardSerial);

/// One of the piles a fight keeps its cards in, which is `PileType`.
///
/// The four a combat has. `Deck` is the run's, and no fight moves a card into
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pile {
    Hand,
    Draw,
    Discard,
    Exhaust,
}

/// A card the fight named, and where it stood when it named it.
///
/// An id alone has no address, and finding one reads every pile. Playing a
/// card is the hottest path there is, so the place travels with the name.
///
/// The place and id name the card together. Whoever acts on this checks that
/// the named card is still at the named place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CardAt<V: ContentVersion> {
    id: CardId<V>,
    pile: Pile,
    index: V::Index,
}

impl<V: ContentVersion> CardAt<V> {
    /// The card a fight found at `index` of `pile`.
    pub fn new(id: CardId<V>, pile: Pile, index: V::Index) -> Self {
        Self { id, pile, index }
    }

    /// Which card this is, whatever has moved since.
    pub fn id(self) -> CardId<V> {
        self.id
    }

    /// The pile it was in.
    pub fn pile(self) -> Pile {
        self.pile
    }

    /// Where it sat in that pile.
    pub fn index(self) -> V::Index {
        self.index
    }
}

impl<V: ContentVersion> CardId<V> {
    /// The card this number names.
    pub fn new(serial: V::CardSerial) -> Self {
        Self(serial)
    }

    /// The number it was minted as.
    pub fn get(self) -> V::CardSerial {
        self.0
    }
}

impl<V: ContentVersion> std::fmt::Display for CardId<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<V: ContentVersion> CardIdent<V> {
    /// A card as it comes out of a pool: nothing upgraded, nothing attached.
    pub const fn new(card: V::Card) -> CardIdent<V> {
        CardIdent {
            card,
            upgrade: 0,
            affliction: None,
            enchantment: None,
        }
    }
}

/// Any one piece of content, when the field it sits in does not say which kind.
///
/// The game's `ModelId` is a category and an entry, because the entry alone is
/// ambiguous. A sum here, and not a category beside an entry, because the
/// category decides which kind the entry is.
///
/// Thirteen of the build's nineteen model categories are here, and the crate
/// README says which six are not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelId<V: ContentVersion> {
    Act(V::Act),
    Affliction(V::Affliction),
    Card(V::Card),
    Character(V::Character),
    Enchantment(V::Enchantment),
    Encounter(V::Encounter),
    Event(V::Event),
    Modifier(V::Modifier),
    Monster(V::Monster),
    Orb(V::Orb),
    Potion(V::Potion),
    Power(V::Power),
    Relic(V::Relic),
}
