//! What starts a run.
//!
//! Not a [`Decision`](crate::Decision), because it is not a move a player makes
//! sitting at the game. It is what has to be true before there is a game to sit
//! at, and [`Play::begin`](crate::Play::begin) is the only thing that takes it.
//!
//! Keeping it out of the decision vocabulary is what lets a run be a type. An
//! implementation holds a run rather than perhaps holding one, and no decision
//! it takes has to answer for a run that never started.

use crate::ContentVersion;
use crate::Seed;

/// The inputs a fresh run needs.
///
/// The same four the game's own `start` command takes. See
/// [PROTOCOL.md](../../../mod/sts2_headless/PROTOCOL.md) for what each means to
/// the real game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start<V: ContentVersion> {
    pub seed: Seed,
    pub character: V::Character,
    pub ascension: V::AscensionLevel,
    /// Run modifiers, which no implementation but the real game accepts yet.
    pub modifiers: Vec<V::Modifier>,
}
