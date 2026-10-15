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
/// The same five the game's own `start` command takes. See
/// [PROTOCOL.md](../../../mod/sts2_headless/PROTOCOL.md) for what each means to
/// the real game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start<V: ContentVersion> {
    pub seed: Seed,
    pub character: V::Character,
    pub ascension: V::AscensionLevel,
    /// Run modifiers, each with what it was set up with.
    pub modifiers: Vec<RunModifier<V>>,
    /// The runs the profile has won with this character, its `TotalWins`.
    /// The Architect picks the dialogue that ends a won run by it.
    pub wins: u32,
}

/// A modifier a run is started with, and what it was set up with.
///
/// On the wire a modifier with nothing set is its id alone, and one with a
/// character is `{"id": ..., "character": ...}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunModifier<V: ContentVersion> {
    pub modifier: V::Modifier,
    /// `CharacterCards.CharacterModel`: the character whose cards it adds.
    /// Nothing for every other modifier.
    pub character: Option<V::Character>,
}

impl<V: ContentVersion> RunModifier<V> {
    /// `modifier`, with nothing set.
    pub fn plain(modifier: V::Modifier) -> Self {
        Self {
            modifier,
            character: None,
        }
    }
}

/// How a [`RunModifier`] is spelled on the wire.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum Spelled<M, C> {
    Plain(M),
    With { id: M, character: C },
}

impl<V: ContentVersion> serde::Serialize for RunModifier<V> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.character {
            None => Spelled::<_, V::Character>::Plain(self.modifier),
            Some(character) => Spelled::With {
                id: self.modifier,
                character,
            },
        }
        .serialize(serializer)
    }
}

impl<'de, V: ContentVersion> serde::Deserialize<'de> for RunModifier<V> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(
            match Spelled::<V::Modifier, V::Character>::deserialize(deserializer)? {
                Spelled::Plain(modifier) => Self::plain(modifier),
                Spelled::With { id, character } => Self {
                    modifier: id,
                    character: Some(character),
                },
            },
        )
    }
}
