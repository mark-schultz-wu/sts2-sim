//! A move the game refused, and why.
//!
//! Not an error. A refusal is a legitimate, deterministic answer that both
//! implementations must give. The differential test's headline finding is the
//! game refusing a move the observation reported as legal. A *fault* is an
//! error, and lives in [`crate::Play::Error`]. That is a socket that died, or
//! an exception inside an adapter.
//!
//! The reason is a type rather than a sentence, and the prose belongs to a
//! front end. The crate README says why, and what the four kinds of reason are.

use crate::{Blocked, CardIdent, ContentVersion, Coord, ModelId, QuestionKind, Undrinkable};

/// A move the game refused.
///
/// Its own type rather than an [`crate::Applied`] with nothing in it, because it
/// is the one answer that means the run did not move. Whatever the caller
/// already had is still current.
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal<V: ContentVersion> {
    /// What was refused, as the implementation names it.
    ///
    /// A string because a [`crate::Decision`] has no name in this crate. The
    /// oracle's `play_card` is the protocol's spelling. It is attribution, not
    /// something to match on. A caller knows what it sent, and the relay that
    /// does not is why this is carried at all.
    pub what: String,
    pub reason: Reason<V>,
}

/// Enough to be an [`Error`](std::error::Error), and no more.
///
/// The reason arrives as its `Debug`, which names a card as `Ironclad(Bash)`.
/// That is this crate's own spelling and not the build's, because this crate
/// does not know the build's. Anything showing a refusal to a person renders
/// [`Refusal::reason`] itself. This is what a `?` prints when nothing else
/// caught it.
impl<V: ContentVersion> std::fmt::Display for Refusal<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "refused {}: {:?}", self.what, self.reason)
    }
}

impl<V: ContentVersion> std::error::Error for Refusal<V> {}

/// Why a move was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum Reason<V: ContentVersion> {
    // ---- wrong moment: the command is fine, the run is not there ----
    /// No run is in progress. Start one.
    NoRun,
    /// The run has ended.
    ///
    /// Winning kills every player too, because `RunManager.WinRun` calls
    /// `GuaranteeKillAllPlayers`. So this carries which it was, rather than
    /// leaving a caller to read it off `alive`.
    RunOver { victory: bool },
    /// A fight is needed and there is none.
    NotInCombat,
    /// A fight is in progress and this cannot be done during one.
    InCombat,
    /// The run is not in the kind of room this needs.
    ///
    /// `found` is nothing for a room this crate has no type for, such as a
    /// combat room, or no room at all.
    WrongRoom {
        wanted: V::RoomType,
        found: Option<V::RoomType>,
    },
    /// The room offers nothing to choose between.
    ///
    /// Beside [`Reason::WrongRoom`] and not the same thing: three kinds of room
    /// take an option, so there is no single kind this wanted.
    NothingToChoose { found: Option<V::RoomType> },
    /// The game is parked on a question, and until it is answered nothing else
    /// will be accepted.
    Pending { kind: QuestionKind },
    /// An answer arrived with nothing to answer.
    NothingPending,

    // ---- no such thing: an index or an id that names nothing ----
    /// An index past what was offered.
    ///
    /// `index` is signed because a refused index is whatever the caller sent,
    /// and a caller that can send `-1` is exactly the caller this refuses.
    OutOfRange { index: i64, count: usize },
    /// A `CombatId` no creature in this fight has.
    NoSuchCreature { id: u32 },

    // ---- the rules say no: the game looked, and said no ----
    /// The card cannot be played at all, for every rule it fails.
    ///
    /// The same two facts the observation reports beside a card in hand, which
    /// is what makes this checkable. A client that played it anyway was acting
    /// on a legal set it had already been given.
    Unplayable {
        card: CardIdent<V>,
        why: V::UnplayableReasons,
        /// Which model said no, for the rule that is one model saying no.
        preventer: Option<ModelId<V>>,
    },
    /// The card can be played and not at that.
    WrongTarget {
        card: CardIdent<V>,
        /// The creature it was pointed at, or nothing for a play with no target
        /// where one was needed.
        target: Option<u32>,
        takes: V::TargetType,
    },
    /// The potion cannot be drunk right now.
    ///
    /// The same two facts the observation reports beside a potion in the belt,
    /// which is what makes this checkable. A client that drank it anyway was
    /// acting on a belt it had already been given.
    Undrinkable { potion: V::Potion, why: Undrinkable },
    /// The potion can be drunk and not at that.
    WrongPotionTarget {
        potion: V::Potion,
        /// The creature it was pointed at, or nothing for a drink with no
        /// target where one was needed.
        target: Option<u32>,
        takes: V::TargetType,
    },
    /// Not a node the run can enter from where it stands.
    NotTravelable { at: Coord },
    /// Nothing to proceed from. This is for leaving a finished boss room.
    CannotProceed,
    /// A rest site option the run cannot take.
    OptionDisabled,
    /// An event option the run has not unlocked.
    OptionLocked,
    /// This chest has already been answered.
    AlreadyAnswered,
    /// A shop entry that will not sell.
    ///
    /// Every number the decision turned on, so a caller can see which of them
    /// was the problem. `blocked` is the third reason beyond stock and gold:
    /// a potion with no free slot.
    Unpurchasable {
        index: u8,
        cost: i32,
        gold: i32,
        stocked: bool,
        blocked: Option<Blocked>,
    },

    // ---- answering a question ----
    /// The same index twice in one answer.
    Duplicate { index: usize },
    /// A selection of a size the question does not take.
    WrongCount { chosen: usize, min: u32, max: u32 },
    /// A reward set that must be answered, not skipped.
    NotSkippable,
    /// A reward already taken. The indices never renumber, so a taken reward
    /// stays in the set at the number it was offered under.
    AlreadyTaken { index: usize },
    /// A card reward taken without saying which card.
    NeedsCardChoice { index: usize, count: usize },
    /// An empty take. Skipping is a different answer and says so.
    EmptyTake,
    /// A cell outside the grid.
    OffGrid {
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    },
    /// A cell already uncovered.
    AlreadyUncovered { x: usize, y: usize },

    // ---- the game's own answer, and the one that should be unreachable ----
    /// The game's own console declined, in its own words.
    ///
    /// Scenario construction goes through `FightConsoleCmd` and
    /// `EventConsoleCmd`, which know why they declined better than an adapter
    /// wrapping them does. Carried verbatim rather than classified, because
    /// classifying it means reading MegaCrit's source and guessing.
    Console { message: String },
    /// A request this crate's types cannot build.
    ///
    /// Unreachable through [`crate::Play`]. That is a missing field, a command
    /// that is not a command, or a number where a name belongs. Reachable
    /// through the oracle's `send_raw`, which is how the refusal path is tested
    /// at all.
    Malformed { message: String },
}
