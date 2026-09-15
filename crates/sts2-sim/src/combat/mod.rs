//! Combat setup and turn resolution.
mod error;
pub use error::Error;

mod board;
pub use board::ResolvedEffect;
mod encounter;
mod legal;
mod live;
mod monster;
mod play;
mod potion;
mod relic;
mod start;
mod stream;
mod turn;
pub(crate) use legal::belt;
pub(crate) use live::Live;
pub(crate) use play::{apply_power, play, resolve};
pub(crate) use potion::drink;
pub(crate) use relic::obtained;
pub(crate) use start::enter;
pub(crate) use turn::end as end_turn;
pub mod pile;

use sts2_core::Rules;

/// What one decision leaves behind.
///
/// The counters travel with the fight because a decision can draw, and the run
/// writes both back together or neither.
pub(crate) struct Fought<V: Rules> {
    pub(crate) live: Live<V>,
    pub(crate) counters: sts2_core::Counters<V>,
    /// The belt as the relics in it left it. They are notified inside the
    /// fight and their models save what they counted, so the run writes these
    /// back beside the fight or neither.
    pub(crate) relics: Vec<crate::run::HeldRelic<V>>,
    /// The last enemy left the fight, so this decision won it.
    pub(crate) won: bool,
}
