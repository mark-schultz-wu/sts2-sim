//! The vocabulary the simulator and both games speak.
//!
//! One set of types that every backend emits, so a differential test is a
//! struct comparison rather than a translation layer, and the same types serve
//! as oracle output, simulator output and RL observation.
//!
//! Three properties the rest of the crate depends on:
//!
//! * **A union of both games, never an intersection.** An intersection would
//!   discard STS1's pile contents and STS2's precomputed damage.
//! * **Every observation carries its [`Provenance`].** A result that cannot say
//!   what produced it is not a result.
//! * **RNG counters are part of the state.** They turn "the states differ" into
//!   "shuffle drifted after 14 draws".
//!
//! Two halves: [`state`] is what a backend reports, [`input`] is what a player
//! sends back. [`diff`] compares two observations; [`mod@env`] is the interface
//! that produces them.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod diff;
pub mod env;
mod game;
pub mod input;
pub mod provenance;
pub mod state;

pub use game::{Game, Sts1, Sts2};
pub use input::{Action, Choice, Input, Selection};
pub use provenance::{BuildId, GameVersion, Provenance, Release};
pub use state::{
    Card, CardRarity, CardType, CombatPile, GameRng, Intent, IntentType, Monster, Obs, Pile,
    RoomType, ScreenType, Stream, Sts1Stream, Sts2Stream,
};
