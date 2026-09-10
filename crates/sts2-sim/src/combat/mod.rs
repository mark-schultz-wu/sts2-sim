//! Combat setup and turn resolution.
mod error;
pub use error::Error;

mod board;
mod encounter;
mod legal;
mod monster;
mod play;
mod start;
mod stream;
mod turn;
pub(crate) use play::{play, resolve};
pub(crate) use start::enter;
pub(crate) use turn::end as end_turn;
pub mod pile;

use monster::Machine;
use sts2_core::{Combat, Rules};

/// A fight as it stands, and what the run keeps beside it.
///
/// A monster's move graph is not in the observation. The observation says what
/// the monster does next, not how the graph arrived there.
#[derive(Debug, Clone)]
pub(crate) struct Ongoing<V: Rules> {
    pub(crate) combat: Combat<V>,
    pub(crate) machines: Vec<Machine<V>>,
}
