//! Where each named generator has got to.
//!
//! Each game names fifteen streams, seeds them from a different kind of value,
//! and runs a different generator underneath. All three differences are
//! declared here rather than centrally, via [`GameRng`].

use core::fmt::Debug;
use core::hash::Hash;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{Game, Sts1, Sts2};

/// How a game does randomness.
pub trait GameRng: Game {
    /// What a stream's generator is seeded with, after the game's per-stream
    /// derivation. STS1 uses a signed 64-bit run seed; STS2 a `uint`.
    type Seed: Copy + Debug + PartialEq + Eq + Serialize + DeserializeOwned;

    /// The generator's internal state. `RandomXS128` is two words, STS2's
    /// `MegaRandom` is a four-word xoshiro256.
    type State: Copy + Debug + PartialEq + Eq + Serialize + DeserializeOwned;

    /// The names of this game's streams.
    type StreamName: Copy + Debug + Ord + Hash + Serialize + DeserializeOwned;
}

impl GameRng for Sts1 {
    type Seed = i64;
    type State = [i64; 2];
    type StreamName = Sts1Stream;
}

impl GameRng for Sts2 {
    type Seed = u32;
    type State = [u64; 4];
    type StreamName = Sts2Stream;
}

/// One stream's position, and three separate checks on it.
///
/// [`seed`](Stream::seed) says whether the stream was *constructed* the same —
/// whether both sides applied the game's derivation rule identically, which is
/// a divergence that predates any draw. [`counter`](Stream::counter) says
/// whether it was *consumed* the same number of times.
/// [`state`](Stream::state) is exact, where a backend can report it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "G: GameRng")]
pub struct Stream<G: GameRng> {
    pub seed: G::Seed,

    /// How many times this stream has been drawn from.
    ///
    /// Counts *calls* to a draw method, not steps of the underlying generator:
    /// bounded draws reject-sample, so one call can consume several generator
    /// outputs while incrementing this once. Both games store it as a 32-bit
    /// int and rebuild a stream by replaying it — STS1's `new Random(seed,
    /// counter)`, STS2's `Rng.FastForwardCounter`.
    pub counter: u32,

    /// The generator's own state, when the backend can report it.
    ///
    /// Exact where `counter` is only a call count, so it catches consumption
    /// divergence that counters agree on. Absent means the side could not say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<G::State>,
}

/// STS1's streams: the thirteen the game names, in `generateSeeds()` order,
/// then the two the determinism mod adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sts1Stream {
    Monster,
    Event,
    Merchant,
    Card,
    Treasure,
    Relic,
    Potion,
    MonsterHp,
    Ai,
    Shuffle,
    CardRandom,
    Misc,
    /// Not in `generateSeeds()`; built per act by the dungeon constructor.
    Map,
    /// The determinism mod's, not the game's: vanilla makes four decisions from
    /// an unseeded generator or a wall clock, and drawing those from one of the
    /// game's thirteen would shift positions vanilla reproduces. Run-scoped.
    UnseededRun,
    /// As [`UnseededRun`](Sts1Stream::UnseededRun), but rebuilt on room entry,
    /// so a fight stays reproducible from `(seed, floor)` alone.
    UnseededCombat,
}

/// STS2's streams: `Entities.Rngs.RunRngType`, shared across the run, then
/// `PlayerRngType`, one set per player.
///
/// The game keeps these in two enums and two sets, `RunRngSet` and
/// `PlayerRngSet`; the prefix preserves which. Which player a `Player*` stream
/// belongs to is [`Obs::player_idx`](crate::Obs::player_idx).
///
/// `Rng.Chaotic` is deliberately absent. Every STS2 stream carries a companion
/// generator of that name, and it is the game's own marker for randomness that
/// must not affect outcomes: its readers are `Nodes.Vfx`, screen and animation
/// code, flavour text, log anonymisation, and `SeedHelper.GetRandomSeed`. An
/// adapter walking `Rng` objects generically would pick it up and emit counters
/// that differ every run by design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sts2Stream {
    RunUpFront,
    RunShuffle,
    RunUnknownMapPoint,
    RunCombatCardGeneration,
    RunCombatPotionGeneration,
    RunCombatCardSelection,
    RunCombatEnergyCosts,
    RunCombatTargets,
    RunMonsterAi,
    RunNiche,
    RunCombatOrbs,
    RunTreasureRoomRelics,
    PlayerRewards,
    PlayerShops,
    PlayerTransformations,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Thirteen named by STS1 plus two from the determinism mod; twelve
    /// `RunRngType` plus three `PlayerRngType`.
    #[test]
    fn each_game_names_fifteen_streams() {
        let sts1 = [
            Sts1Stream::Monster,
            Sts1Stream::Event,
            Sts1Stream::Merchant,
            Sts1Stream::Card,
            Sts1Stream::Treasure,
            Sts1Stream::Relic,
            Sts1Stream::Potion,
            Sts1Stream::MonsterHp,
            Sts1Stream::Ai,
            Sts1Stream::Shuffle,
            Sts1Stream::CardRandom,
            Sts1Stream::Misc,
            Sts1Stream::Map,
            Sts1Stream::UnseededRun,
            Sts1Stream::UnseededCombat,
        ];
        let sts2 = [
            Sts2Stream::RunUpFront,
            Sts2Stream::RunShuffle,
            Sts2Stream::RunUnknownMapPoint,
            Sts2Stream::RunCombatCardGeneration,
            Sts2Stream::RunCombatPotionGeneration,
            Sts2Stream::RunCombatCardSelection,
            Sts2Stream::RunCombatEnergyCosts,
            Sts2Stream::RunCombatTargets,
            Sts2Stream::RunMonsterAi,
            Sts2Stream::RunNiche,
            Sts2Stream::RunCombatOrbs,
            Sts2Stream::RunTreasureRoomRelics,
            Sts2Stream::PlayerRewards,
            Sts2Stream::PlayerShops,
            Sts2Stream::PlayerTransformations,
        ];
        fn distinct<T: Ord>(mut v: alloc::vec::Vec<T>) -> usize {
            v.sort_unstable();
            v.dedup();
            v.len()
        }
        assert_eq!(distinct(alloc::vec::Vec::from(sts1)), 15);
        assert_eq!(distinct(alloc::vec::Vec::from(sts2)), 15);
    }

    /// A stream name keys a map, so it must be a plain string on the wire, and
    /// the two games' names live in separate types rather than one namespace.
    #[test]
    fn stream_names_are_bare_strings_per_game() {
        assert_eq!(
            serde_json::to_string(&Sts1Stream::Shuffle).expect("serialises"),
            r#""shuffle""#
        );
        assert_eq!(
            serde_json::to_string(&Sts2Stream::RunCombatCardGeneration).expect("serialises"),
            r#""run_combat_card_generation""#
        );
        assert!(serde_json::from_str::<Sts1Stream>(r#""run_shuffle""#).is_err());
        assert!(serde_json::from_str::<Sts2Stream>(r#""card_random""#).is_err());
    }

    /// The seed types differ, so an STS2 seed is not a valid STS1 one.
    #[test]
    fn a_seed_is_the_kind_its_game_uses() {
        let sts1: Stream<Sts1> =
            serde_json::from_str(r#"{"seed":-42,"counter":3}"#).expect("i64 seed");
        assert_eq!(sts1.seed, -42);
        assert_eq!(sts1.state, None);

        assert!(
            serde_json::from_str::<Stream<Sts2>>(r#"{"seed":-42,"counter":3}"#).is_err(),
            "a negative seed is not a uint"
        );
    }

    #[test]
    fn state_is_the_generator_a_game_actually_uses() {
        let s: Stream<Sts2> =
            serde_json::from_str(r#"{"seed":7,"counter":1,"state":[1,2,3,4]}"#).expect("4 words");
        assert_eq!(s.state, Some([1u64, 2, 3, 4]));
        assert!(
            serde_json::from_str::<Stream<Sts1>>(r#"{"seed":7,"counter":1,"state":[1,2,3,4]}"#)
                .is_err(),
            "RandomXS128 is two words, not four"
        );
    }
}
