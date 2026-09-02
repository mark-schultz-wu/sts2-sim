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
use crate::ContentVersion;
use crate::Coord;
use crate::Seed;
use nonmax::NonMaxU32;

/// One decision in a run.
///
/// Two commands the real game accepts are absent. `observe` reports state
/// without advancing, and a deterministic game between commands cannot answer
/// it twice differently. `quit` ends the session, not the run, so it is `Drop`
/// on an implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision<V: ContentVersion> {
    /// Starts a fresh run.
    ///
    /// Not a move within a run, which is the honest objection to its being here.
    /// It stays because it keeps a recording uniform, every line being a
    /// `Step`, and [`crate::Play`] one method wide.
    Start {
        seed: Seed,
        character: V::Character,
        ascension: V::AscensionLevel,
        modifiers: Vec<V::Modifier>,
    },
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
        target: Option<NonMaxU32>,
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
        target: Option<NonMaxU32>,
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
