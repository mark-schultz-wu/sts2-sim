//! Everything a run draws before play begins.

mod fight;
mod setup;
mod shop;
mod state;
mod travel;

pub use setup::{Opening, Rarity, RelicBag, run_acts};
pub(crate) use state::{BELT, Belt, HeldRelic, State};

/// What one command decided, or the refusal that stopped it.
///
/// A command reports this and nothing else. An observation costs more than
/// most commands do. So it is built once, at the boundary, and only for a
/// caller that asked to see one.
pub type Settled<V> = Result<sts2_core::Decided, sts2_core::Refusal<V>>;
