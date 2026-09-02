//! What a caller sends.
//!
//! Split three ways, and the split is the point rather than tidiness.
//!
//! A [`Decision`] is something a player does sitting at the game: play a card,
//! take a rest site option, travel.
//!
//! An [`Answer`] is the subset that only exists while the game is parked on a
//! [`Question`](crate::Question). That is why it is its own type rather than
//! more variants of `Decision`.
//!
//! A [`Setup`] is not a move at all. It constructs a situation no run reaches by
//! playing. It is on [`Construct`](crate::Construct) rather than on
//! [`Play`](crate::Play), so that an implementation which cannot build one says
//! so in its type instead of at runtime.
//!
//! [`Step`] is either of the two that can be recorded, and is what this module
//! is named for. A recording is a sequence of them. Replaying one into an
//! implementation that can only play fails at the first [`Setup`]. A constructed
//! run cannot be reproduced by something that cannot construct.

pub mod answer;
pub mod decision;
pub mod setup;

use crate::ContentVersion;
use crate::{Decision, Setup};

/// A decision, or a piece of scenario construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step<V: ContentVersion> {
    Decision(Decision<V>),
    Setup(Setup<V>),
}

impl<V: ContentVersion> From<Decision<V>> for Step<V> {
    fn from(d: Decision<V>) -> Self {
        Self::Decision(d)
    }
}

impl<V: ContentVersion> From<Setup<V>> for Step<V> {
    fn from(s: Setup<V>) -> Self {
        Self::Setup(s)
    }
}
