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
/// through [`Play::begin`](crate::Play::begin). `observe` reports state without
/// advancing, and a deterministic game between commands cannot answer it twice
/// differently. `quit` ends the session, not the run, so it is `Drop` on an
/// implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        index: u8,
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
        index: u8,
        target: Option<CombatId>,
    },
    /// Indexes into the options the *most recent* observation reported. The rest
    /// site list shrinks as it is chosen from, so an older index means nothing.
    ChooseOption {
        index: u8,
    },
    /// Indexes into the shop entries the most recent observation reported.
    Buy {
        index: u8,
    },
    /// Responds to a question a parked game asked.
    Answer(Answer),
}
