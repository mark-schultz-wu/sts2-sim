//! Every random stream a run keeps, as the generators themselves.
//!
//! A stream is four words and a draw count. `SerializableRng` is the game's own
//! save format for one, and carries both. A run here carries the generator, so
//! reaching a stream is a move rather than a replay.
//!
//! Rebuilding a stream from its count means drawing to where it stood, which
//! costs as much as the run has drawn. Doing that per decision makes the work
//! quadratic in the run. A search rolling out from a deep position pays it
//! again every rollout.
//!
//! [`Counters`] is the report's shape, and what a divergence is judged on. It
//! is read off these on the way out, and never read back in.

use std::marker::PhantomData;

use smallvec::SmallVec;
use sts2_core::rng::GameRng;
use sts2_core::{Counters, Rules};

use crate::rng::MegaRng;

/// The run's twelve streams and the player's three, at the positions they stand.
///
/// Held at the position each stream has in [`Rules::run_rng_types`], so this is
/// a fixed row of generators and not a map. Cloning one copies that row, which
/// is what a decision does before it draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Streams<V: Rules> {
    run: SmallVec<[MegaRng; 16]>,
    player: SmallVec<[MegaRng; 4]>,
    version: PhantomData<fn() -> V>,
}

impl<V: Rules> Streams<V> {
    /// Every stream as the run's seed names it, before anything has drawn.
    pub(crate) fn opening(raw_seed: u32) -> Self {
        Self {
            run: V::run_rng_types()
                .iter()
                .map(|&kind| MegaRng::for_stream(raw_seed, kind))
                .collect(),
            player: V::player_rng_types()
                .iter()
                .map(|&kind| MegaRng::for_stream(raw_seed, kind))
                .collect(),
            version: PhantomData,
        }
    }

    /// One of the run's streams, where it stands.
    ///
    /// Handed out rather than lent, because a rule draws from two or three at
    /// once and they are rows of one table. What it drew is put back with
    /// [`Streams::record`], so a decision that fails part way leaves the run's
    /// own streams where they were.
    pub(crate) fn stream(&self, kind: V::RunRngType) -> MegaRng {
        self.run[slot(V::run_rng_types(), kind)].clone()
    }

    /// Where a stream now stands, after a rule drew from it.
    pub(crate) fn record(&mut self, kind: V::RunRngType, rng: MegaRng) {
        self.run[slot(V::run_rng_types(), kind)] = rng;
    }

    /// One of the run's streams, to draw from in place.
    pub(crate) fn run(&mut self, kind: V::RunRngType) -> &mut MegaRng {
        &mut self.run[slot(V::run_rng_types(), kind)]
    }

    /// One of the player's streams, to draw from in place.
    pub(crate) fn player(&mut self, kind: V::PlayerRngType) -> &mut MegaRng {
        &mut self.player[slot(V::player_rng_types(), kind)]
    }

    /// How far each stream has been drawn, which is what a report carries.
    pub(crate) fn counters(&self) -> Counters<V> {
        Counters {
            run: V::run_rng_types()
                .iter()
                .copied()
                .zip(self.run.iter().map(counter_of))
                .collect(),
            player: V::player_rng_types()
                .iter()
                .copied()
                .zip(self.player.iter().map(counter_of))
                .collect(),
        }
    }
}

/// Where in a row a stream's generator is kept.
///
/// Every stream the build has is in its own list, so one that is not is a
/// stream the build did not generate.
fn slot<K: Eq>(kinds: &'static [K], kind: K) -> usize {
    kinds
        .iter()
        .position(|candidate| *candidate == kind)
        .expect("a stream is one of the build's own")
}

/// A reported count as a `u32`, which is the shape the wire uses.
fn counter_of(rng: &MegaRng) -> u32 {
    u32::try_from(rng.counter()).expect("a stream's draw count is not negative")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_content::v0_107_1::{V0_107_1, runstate::RunRngType};
    use sts2_core::StreamKinds;

    /// Every stream opens on its own name, and no two open alike.
    ///
    /// The generators are a row read by position. A row in another order than
    /// [`Rules::run_rng_types`] hands every stream its neighbour's seed.
    /// Nothing downstream tells the difference: the draws are real numbers off
    /// a real stream, and the wrong one.
    #[test]
    fn every_stream_opens_on_its_own_name() {
        let mut streams = Streams::<V0_107_1>::opening(123);

        for kind in RunRngType::ALL {
            assert_eq!(
                *streams.run(kind),
                MegaRng::for_stream(123, kind),
                "{} opened on another stream's seed",
                kind.name()
            );
        }
    }

    /// A draw moves one stream and leaves the rest where they were.
    #[test]
    fn drawing_moves_one_stream() {
        let mut streams = Streams::<V0_107_1>::opening(123);

        let mut shuffle = streams.stream(RunRngType::Shuffle);
        for _ in 0..9 {
            shuffle.next_u64();
        }
        streams.record(RunRngType::Shuffle, shuffle);

        let counters = streams.counters();
        assert_eq!(counters.run[&RunRngType::Shuffle], 9);
        for kind in RunRngType::ALL
            .into_iter()
            .filter(|k| *k != RunRngType::Shuffle)
        {
            assert_eq!(counters.run[&kind], 0, "{} moved too", kind.name());
        }
    }

    /// A stream handed out and not recorded leaves the run where it was.
    ///
    /// What makes a decision transactional: it draws from a copy, and the run
    /// takes the copy only once the decision is whole.
    #[test]
    fn a_stream_that_is_not_recorded_does_not_move() {
        let streams = Streams::<V0_107_1>::opening(123);

        let mut taken = streams.stream(RunRngType::Niche);
        taken.next_u64();

        assert_eq!(streams.counters().run[&RunRngType::Niche], 0);
    }
}
