//! Combat setup and turn resolution.
mod error;
pub use error::Error;

mod board;
mod encounter;
mod legal;
mod live;
mod monster;
mod play;
mod start;
mod stream;
mod turn;
pub(crate) use legal::belt;
pub(crate) use live::Live;
pub(crate) use play::{apply_power, play, resolve};
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
    /// The last enemy left the fight, so this decision won it.
    pub(crate) won: bool,
}

/// `CombatManager.EndCombat`: what a won fight leaves in the observation.
///
/// The fight is over and still reported. The piles empty into nothing rather
/// than into each other. The turn stops where it stopped. The stars stay,
/// because they belong to the run and not to the turn.
pub(crate) fn finish<V: Rules>(live: &mut Live<V>) {
    live.phase = <V::PlayerTurnPhase as sts2_core::TurnPhaseKinds>::NONE;
    live.energy = num_traits::zero();
    live.hand.clear();
    live.draw.clear();
    live.discard.clear();
    live.exhaust.clear();
    // `Player.AfterCombatEnd`: every power off, and the block with it. Nothing
    // a fight put on a creature outlives the fight.
    for creature in &mut live.creatures {
        creature.powers.clear();
        creature.block = num_traits::zero();
    }
}
