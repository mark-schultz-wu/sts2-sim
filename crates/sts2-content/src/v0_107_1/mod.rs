//! Content for game build `v0.107.1`.
//!
//! `dumps/` holds what the running game printed — `STS2_HEADLESS_MODE=carddump`
//! and friends, see `mod/sts2_dump/`. Each dump carries the content
//! hash in its header, and the `tests` module requires that to match [`VERSION`], so
//! regenerating against a different build fails rather than quietly replacing
//! the tables. `generated/` mirrors it, one module per kind of content.
//!
//! Static means true with no run in progress: what a card costs, how much HP a
//! monster rolls, whether a power stacks, which monsters an encounter fields.
//! What any of it *does* is code and gets ported by hand.
//!
//! Where a value might be code, the dump says so rather than guessing, and it
//! generates as [`Field::Computed`](crate::Field) — the simulator's cue to ask
//! rather than look up. Each domain decides that differently, because each had
//! to: a card is marked from its overrides, a power from its IL, an encounter
//! by running it and reading the `Rng` counter afterwards. All of them err the
//! same way, towards computed, because marking a constant as computed costs one
//! method and the reverse costs a divergence nobody can see.

mod generated;

pub use generated::*;

use crate::Version;

pub const VERSION: Version = Version {
    name: "v0.107.1",
    content_hash: 3_954_186_980,
};

/// The card dump, verbatim — test data, not runtime data.
///
/// `cargo xtask codegen` reads it from disk to build [`cards`], and the `tests` module
/// replays it to check that table. Nothing at runtime should parse it: that is
/// what the generated table is for, and a `pub const` here would both invite it
/// and put the dump's text into every binary that links this crate.
#[cfg(test)]
const CARDS: &str = include_str!("dumps/cards.txt");

/// The monster dump, verbatim — test data, on the same terms as [`CARDS`].
#[cfg(test)]
const MONSTERS: &str = include_str!("dumps/monsters.txt");

/// The power dump, verbatim — test data, on the same terms as [`CARDS`].
#[cfg(test)]
const POWERS: &str = include_str!("dumps/powers.txt");

/// The encounter dump, verbatim — test data, on the same terms as [`CARDS`].
#[cfg(test)]
const ENCOUNTERS: &str = include_str!("dumps/encounters.txt");

#[cfg(test)]
mod tests;
