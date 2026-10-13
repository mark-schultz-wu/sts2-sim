//! Combat setup and turn resolution.
mod error;
pub use error::Error;

mod board;
mod cards;
mod piles;
pub use board::ResolvedEffect;
pub(crate) use board::{exact, whole};
mod encounter;
mod hook;
mod legal;
mod listen;
mod live;
mod monster;
mod observed;
mod play;
mod potion;
mod powers;
mod relic;
mod start;
mod turn;
pub(crate) use legal::{belt, drinks, moves as legal_moves};
pub(crate) use live::{ExtraReward, Heard, Live, printed_costs};
pub(crate) use observed::Fight;
pub(crate) use play::{answer, apply_power, belt_changed, enchant, grant_card, play, resolve};
pub(crate) use potion::drink;
pub(crate) use relic::obtained;
pub(crate) use start::Entering;
pub(crate) use start::{Entered, enter, roll_layout};
pub(crate) use turn::end as end_turn;
pub mod pile;

use sts2_core::Rules;

/// What a decision in a fight is handed, all borrowed out of the run.
///
/// A decision works on these where they stand. One the simulator cannot carry
/// out is a fault, and [`sts2_core::Play`] makes no promise about a run after
/// a fault. A refusal is decided before any of this is touched.
///
/// An auto-play runs inside a board, and lends that board's streams.
pub(crate) struct Fighting<'a, V: Rules, S = crate::stream::Streams<V>> {
    pub(crate) live: &'a mut Live<V>,
    pub(crate) streams: &'a mut S,
    /// The belt. Relics are notified inside the fight and their models save
    /// what they counted.
    pub(crate) relics: &'a mut crate::run::Belt<V>,
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

/// `CheckWinCondition` once a decision is over: `ActionExecutor` asks it
/// after each action, and `StartTurn` once the setup is done. It reads the
/// fight as it stands, so a win that something in the decision did not hand
/// back still ends it. A decision waiting on a question is not over yet.
pub(crate) fn check_win_condition<V: Rules, S>(fighting: Fighting<'_, V, S>) -> Option<Ended> {
    let Fighting { live, relics, .. } = fighting;
    if !live.won || live.asking.is_some() {
        return None;
    }
    live.won = false;
    if !board::ending::<V>(listen::Walk { live, relics }) {
        return None;
    }
    listen::combat_ended::<V>(live, relics);
    Some(Ended::Won)
}
