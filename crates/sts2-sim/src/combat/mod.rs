//! Combat setup and turn resolution.
mod error;
pub use error::Error;

mod board;
pub use board::ResolvedEffect;
mod encounter;
mod hook;
mod legal;
mod live;
mod monster;
mod play;
mod potion;
mod power;
mod relic;
mod start;
mod turn;
pub(crate) use legal::belt;
pub(crate) use live::Live;
pub(crate) use play::{answer, apply_power, play, resolve};
pub(crate) use potion::drink;
pub(crate) use relic::obtained;
pub(crate) use start::enter;
pub(crate) use turn::end as end_turn;
pub mod pile;

use sts2_core::Rules;

/// What one decision leaves behind.
///
/// The streams travel with the fight because a decision can draw, and the run
/// writes both back together or neither.
pub(crate) struct Fought<V: Rules> {
    pub(crate) live: Live<V>,
    pub(crate) streams: crate::stream::Streams<V>,
    /// The belt as the relics in it left it. They are notified inside the
    /// fight and their models save what they counted, so the run writes these
    /// back beside the fight or neither.
    pub(crate) relics: Vec<crate::run::HeldRelic<V>>,
    /// How this decision ended the fight, for one that ended it.
    pub(crate) ended: Option<Ended>,
}

/// How a fight finished.
///
/// `CombatManager` keeps the two apart: `IsEnding` asks whether any primary
/// enemy is alive, and `_pendingLoss` is set when the last player dies. A
/// decision reaches at most one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ended {
    Won,
    Lost,
}
