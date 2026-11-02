//! What a player does.
//!
//! A decision vocabulary, and nothing else. There is no serialization here on
//! purpose. How a decision is spelled on a socket is the oracle's business and
//! lives in `sts2-headless`, the way the mod's spelling of an observation does.
//! Both implementations accept these, and a [`crate::Recording`] is a list of
//! them.
//!
//! See [PROTOCOL.md](../../../mod/sts2_headless/PROTOCOL.md) for what each
//! argument means to the real game.

use crate::Answer;
use crate::CombatId;
use crate::ContentVersion;
use crate::Coord;

/// One decision in a run.
///
/// Three commands the real game accepts are absent. `start` is not a move
/// within a run, so it is [`Start`](crate::Start) and reaches an implementation
/// through [`RunTable::begin`](crate::RunTable::begin). `observe` reports state without
/// advancing, and a deterministic game between commands cannot answer it twice
/// differently. `quit` ends the session, not the run, so it is `Drop` on an
/// implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision<V: ContentVersion> {
    EnterRoom {
        room: V::RoomType,
    },
    Travel {
        to: Coord,
    },
    /// Leaves a finished act for the next one.
    ///
    /// Not a map move, because an act's boss sits outside the grid with no
    /// children. Once it is beaten the map correctly reports nowhere to go, and
    /// this is the only way onward. Legal exactly when `map.proceed` is true.
    Proceed,
    /// `index` is into the hand. `target` is a creature's `CombatId`, not an
    /// index into the living creatures, which shifts when one dies.
    PlayCard {
        index: HandIndex,
        target: Option<CombatId>,
    },
    EndTurn,
    /// Drinks one potion from the belt, addressed by its index in the
    /// observation.
    ///
    /// `target` is a creature's `CombatId`, as [`Decision::PlayCard`]'s is. It
    /// is absent for a potion that takes no target, and for any potion drunk
    /// outside combat. There are no combat ids to name there, and the game
    /// points it at the player itself.
    UsePotion {
        index: BeltIndex,
        target: Option<CombatId>,
    },
    /// Indexes into the options the *most recent* observation reported. The rest
    /// site list shrinks as it is chosen from, so an older index means nothing.
    ///
    /// `target` is the player a co-op rest option is aimed at, by seat, as
    /// Mend's is. It is absent for every other option.
    ChooseOption {
        index: u8,
        target: Option<crate::SeatIndex>,
    },
    /// Indexes into the shop entries the most recent observation reported.
    Buy {
        index: u8,
    },
    /// Responds to a question a parked game asked.
    Answer(Answer),
    /// Throws away the potion at `index` in the belt, as its popup's discard
    /// button does: `DiscardPotionGameAction`, which runs `PotionCmd.Discard`.
    ///
    /// Last, because a digest hashes a decision by its place in this list.
    DiscardPotion {
        index: BeltIndex,
    },
}

/// Where a card sits in the hand, left to right.
///
/// A byte, because a hand the game deals is far shorter than that. The type is
/// separate from [`BeltIndex`] because the two name different lists, and a
/// number that names one names nothing in the other.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct HandIndex(nonmax::NonMaxU8);

impl std::fmt::Display for HandIndex {
    /// The number a front end says when it names the card.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl HandIndex {
    /// The place this number names.
    pub fn new(index: nonmax::NonMaxU8) -> Self {
        Self(index)
    }

    /// The place this number names, or [`crate::OutOfRange`] for one no hand
    /// holds.
    pub fn try_new(index: u8) -> Result<Self, crate::OutOfRange> {
        nonmax::NonMaxU8::new(index)
            .map(Self)
            .ok_or(crate::OutOfRange)
    }

    /// The number, for the edges that spell a place or read one.
    pub fn get(self) -> u8 {
        self.0.get()
    }
}

/// Where a potion sits in the belt, left to right.
///
/// A byte, for the reason [`HandIndex`] is one. The belt is shorter still: the
/// game starts a run with three slots.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct BeltIndex(nonmax::NonMaxU8);

impl std::fmt::Display for BeltIndex {
    /// The number a front end says when it names the potion.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl BeltIndex {
    /// The slot this number names.
    pub fn new(index: nonmax::NonMaxU8) -> Self {
        Self(index)
    }

    /// The slot this number names, or [`crate::OutOfRange`] for one no belt
    /// has.
    pub fn try_new(index: u8) -> Result<Self, crate::OutOfRange> {
        nonmax::NonMaxU8::new(index)
            .map(Self)
            .ok_or(crate::OutOfRange)
    }

    /// The number, for the edges that spell a slot or read one.
    pub fn get(self) -> u8 {
        self.0.get()
    }
}
