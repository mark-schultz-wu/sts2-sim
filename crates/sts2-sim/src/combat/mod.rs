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
pub(crate) use board::reported;
pub(crate) use legal::belt;
pub(crate) use play::{apply_power, play, resolve};
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
    /// The debuffs on the player whose next tick is skipped. See
    /// [`board::Board::skipped`].
    pub(crate) skipping: Vec<(sts2_core::CombatId, V::Power)>,
}

/// What one decision leaves behind.
///
/// The counters travel with the fight because a decision can draw, and the run
/// writes both back together or neither.
pub(crate) struct Fought<V: Rules> {
    pub(crate) ongoing: Ongoing<V>,
    pub(crate) counters: sts2_core::Counters<V>,
    /// The last enemy left the fight, so this decision won it.
    pub(crate) won: bool,
}

/// `CombatManager.EndCombat`: what a won fight leaves in the observation.
///
/// The fight is over and still reported. The piles empty into nothing rather
/// than into each other. The turn stops where it stopped. The stars stay,
/// because they belong to the run and not to the turn.
pub(crate) fn finish<V: Rules>(combat: &mut Combat<V>) {
    combat.phase = <V::PlayerTurnPhase as sts2_core::TurnPhaseKinds>::NONE;
    combat.energy = num_traits::zero();
    combat.hand.clear();
    combat.draw.clear();
    combat.discard.clear();
    combat.exhaust.clear();
    combat.legal.clear();
    // `Player.AfterCombatEnd`: every power off, and the block with it. Nothing
    // a fight put on a creature outlives the fight.
    for creature in &mut combat.creatures {
        creature.powers.clear();
        creature.block = num_traits::zero();
    }
}
