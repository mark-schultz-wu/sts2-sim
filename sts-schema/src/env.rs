//! The stepping interface, and what an agent must implement.
//!
//! # Where randomness lives
//!
//! A [`Policy`] is `&mut self`, because agents carry state — a network, a
//! search tree, per-turn counters — but its randomness is passed in rather than
//! owned. An agent holding a private generator is a hidden input: it looks
//! deterministic and replays differently. With the caller supplying it, same
//! seed plus same agent replays by construction.
//!
//! That generator is deliberately not one of the game's streams. An agent
//! drawing from `shuffleRng` would shift every later reshuffle in the run it is
//! playing.
//!
//! # Cost
//!
//! [`Obs`] is a report type: owned strings, maps, vectors. That suits
//! differential testing and RL observations, and is far too expensive inside a
//! tree search, which wants the raw state and a `Copy` action.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use rand::Rng;
use rand::seq::IndexedRandom;

use crate::provenance::GameVersion;
use crate::{GameRng, Input, Obs};

/// Rejected action, with enough context to say why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IllegalAction {
    pub action: Input,
    pub reason: String,
}

impl core::fmt::Display for IllegalAction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "illegal action {:?}: {}", self.action, self.reason)
    }
}

impl core::error::Error for IllegalAction {}

/// Why a step failed.
///
/// The two differ in what a caller can do about them: a rejected action can be
/// recovered from by choosing again, a dead backend cannot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepError {
    Illegal(IllegalAction),
    /// The game or adapter failed: container died, pipe closed, protocol error.
    Backend(String),
}

impl core::fmt::Display for StepError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StepError::Illegal(e) => write!(f, "{e}"),
            StepError::Backend(d) => write!(f, "backend failure: {d}"),
        }
    }
}

impl core::error::Error for StepError {}

impl From<IllegalAction> for StepError {
    fn from(e: IllegalAction) -> Self {
        StepError::Illegal(e)
    }
}

/// Something that can be stepped: the simulator, or an adapter over a game.
///
/// Every backend implements this and emits the same [`Obs`], so a differential
/// test is a struct comparison.
///
/// # The diffing interface, not the search interface
///
/// Observing allocates, at roughly 2.5 µs against ~95 ns for the underlying
/// engine call. That is the right trade against oracles running at 6–163
/// steps/s, and the wrong one for search or training, which should call the
/// engine directly.
pub trait SpireEnv {
    /// Which game this backend is.
    type Game: GameRng + GameVersion;

    /// Current decision point.
    fn observe(&self) -> Obs<Self::Game>;

    /// Legal inputs, arguments included, so a caller never has to guess one.
    fn legal_actions(&self) -> Vec<Input>;

    /// Advance one quiescent step, mutating state.
    ///
    /// Returns `()` so that stepping does not force an observation, which
    /// allocates. A differential test calls `step` then
    /// [`observe`](SpireEnv::observe); a search observes only when it wants a
    /// report.
    fn step(&mut self, action: &Input) -> Result<(), StepError>;

    /// Whether the episode has ended.
    fn done(&self) -> bool {
        self.observe().done
    }
}

/// What an agent implements.
///
/// The generator arrives as `&mut dyn Rng` rather than a type parameter to keep
/// this dyn-compatible, since a harness normally holds a `Box<dyn Policy>`.
/// `rand`'s extension methods are blanket-implemented for `RngCore + ?Sized`,
/// so implementors lose nothing.
pub trait Policy<G: GameRng + GameVersion> {
    fn act(&mut self, obs: &Obs<G>, legal: &[Input], rng: &mut dyn Rng) -> Input;

    /// Human-readable, for diff reports and experiment logs.
    fn name(&self) -> &str {
        "policy"
    }
}

/// Always the first enumerated action. The dullest possible baseline.
#[derive(Clone, Debug, Default)]
pub struct FirstLegal;

impl<G: GameRng + GameVersion> Policy<G> for FirstLegal {
    fn act(&mut self, _obs: &Obs<G>, legal: &[Input], _rng: &mut dyn Rng) -> Input {
        legal[0].clone()
    }
    fn name(&self) -> &str {
        "first-legal"
    }
}

/// Uniform over legal inputs, and reproducible for a given seed because the
/// generator is the caller's.
#[derive(Clone, Debug, Default)]
pub struct Uniform;

impl<G: GameRng + GameVersion> Policy<G> for Uniform {
    fn act(&mut self, _obs: &Obs<G>, legal: &[Input], rng: &mut dyn Rng) -> Input {
        legal
            .choose(rng)
            .expect("no legal actions to choose from")
            .clone()
    }
    fn name(&self) -> &str {
        "uniform"
    }
}

/// Why a rollout stopped. A harness that cannot tell "finished" from "stalled"
/// will eventually report a stall as a pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    Done,
    NoLegalActions,
    Budget,
    Illegal,
    Backend,
}

/// Drive an environment to completion or a step budget, collecting the trace.
pub fn rollout<E: SpireEnv, P: Policy<E::Game>>(
    env: &mut E,
    policy: &mut P,
    rng: &mut dyn Rng,
    budget: usize,
) -> (Vec<Obs<E::Game>>, StopReason) {
    let mut trace = vec![env.observe()];
    for _ in 0..budget {
        if env.done() {
            return (trace, StopReason::Done);
        }
        let legal = env.legal_actions();
        if legal.is_empty() {
            return (trace, StopReason::NoLegalActions);
        }
        let obs = trace.last().expect("seeded above").clone();
        let action = policy.act(&obs, &legal, rng);
        match env.step(&action) {
            Ok(()) => trace.push(env.observe()),
            Err(StepError::Illegal(_)) => return (trace, StopReason::Illegal),
            Err(StepError::Backend(_)) => return (trace, StopReason::Backend),
        }
    }
    (trace, StopReason::Budget)
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    use super::*;
    use crate::{Choice, Selection, Sts1};

    fn act(n: u8) -> Input {
        Input::Choice(Choice::PickOptions {
            positions: Selection::one(n),
        })
    }

    fn obs() -> Obs<Sts1> {
        serde_json::from_str(
            r#"{"provenance":{"game":"sts1","game_version":1,"adapter":"test","mods":[]},
                "screen":"map","legal":[],"floor":0,"act":1,"hp":1,"max_hp":1,"gold":0}"#,
        )
        .expect("fixture parses")
    }

    /// The property the module docs claim, and a property of the caller
    /// holding the generator rather than of `rand`.
    fn choices(seed: u64) -> Vec<Input> {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut policy = Uniform;
        let legal: Vec<Input> = (0..8u8).map(act).collect();
        (0..32)
            .map(|_| policy.act(&obs(), &legal, &mut rng))
            .collect()
    }

    #[test]
    fn same_seed_replays_identically() {
        assert_eq!(choices(1), choices(1));
    }

    #[test]
    fn different_seeds_diverge() {
        assert_ne!(choices(1), choices(2));
    }

    #[test]
    fn uniform_reaches_every_action() {
        let seen: alloc::collections::BTreeSet<_> = choices(7)
            .into_iter()
            .map(|a| alloc::format!("{a}"))
            .collect();
        assert_eq!(seen.len(), 8, "some action was never selected: {seen:?}");
    }

    #[test]
    fn first_legal_ignores_the_generator() {
        let legal: Vec<Input> = (0..4u8).map(act).collect();
        let mut rng = SmallRng::seed_from_u64(3);
        assert_eq!(FirstLegal.act(&obs(), &legal, &mut rng), legal[0]);
    }
}
