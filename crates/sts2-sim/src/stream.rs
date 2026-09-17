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
//! [`Positions`] is the report's shape, and what a divergence is judged on. It
//! is read off these on the way out, and never read back in.

use std::marker::PhantomData;

use smallvec::SmallVec;
use sts2_core::rng::GameRng;
use sts2_core::{Position, Positions, Rules, RunStreamKinds};

use crate::rng::MegaRng;

/// The run's twelve streams and the player's three, at the positions they stand.
///
/// Held at the position each stream has in [`Rules::run_rng_types`], so this is
/// a fixed row of generators and not a map. Cloning one copies that row, which
/// is what a decision does before it draws.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Streams<V: Rules> {
    run: SmallVec<[MegaRng; 16]>,
    player: SmallVec<[MegaRng; 4]>,
    version: PhantomData<fn() -> V>,
}

impl<V: Rules> Clone for Streams<V> {
    /// Copies the two rows rather than walking them.
    ///
    /// `SmallVec` clones a list one item at a time, because its `Clone` cannot
    /// see that the item is `Copy`. A generator is 48 bytes and there are
    /// fifteen, and a decision copies them all before it draws.
    fn clone(&self) -> Self {
        Self {
            run: SmallVec::from_slice(&self.run),
            player: SmallVec::from_slice(&self.player),
            version: PhantomData,
        }
    }
}

impl<V: Rules> Streams<V> {
    /// Every stream as the run's seed names it, before anything has drawn.
    pub(crate) fn opening(raw_seed: u64) -> Self {
        Self {
            run: V::run_rng_types()
                .iter()
                .map(|&kind| sts2_core::rng::for_stream::<V, MegaRng>(raw_seed, kind))
                .collect(),
            player: V::player_rng_types()
                .iter()
                .map(|&kind| sts2_core::rng::for_stream::<V, MegaRng>(raw_seed, kind))
                .collect(),
            version: PhantomData,
        }
    }

    /// The streams a fight draws through, lent together.
    ///
    /// A rule draws from two or three at once, and they are rows of one table.
    /// So one call lends the set rather than each row separately. Drawing moves
    /// the row where it lies, and there is nothing to put back.
    pub(crate) fn combat(&mut self) -> CombatRngs<'_> {
        let kinds = V::run_rng_types();
        let [shuffle, niche, generation, ai] = self
            .run
            .get_disjoint_mut([
                slot(kinds, V::RunRngType::SHUFFLE),
                slot(kinds, V::RunRngType::NICHE),
                slot(kinds, V::RunRngType::COMBAT_CARD_GENERATION),
                slot(kinds, V::RunRngType::MONSTER_AI),
            ])
            .expect("a build names each of its streams once");
        CombatRngs {
            shuffle,
            niche,
            generation,
            ai,
        }
    }

    /// One of the run's streams, to draw from in place.
    pub(crate) fn run(&mut self, kind: V::RunRngType) -> &mut MegaRng {
        &mut self.run[slot(V::run_rng_types(), kind)]
    }

    /// One of the player's streams, to draw from in place.
    pub(crate) fn player(&mut self, kind: V::PlayerRngType) -> &mut MegaRng {
        &mut self.player[slot(V::player_rng_types(), kind)]
    }

    /// Where each stream stands, which is what a report carries.
    ///
    /// A run holds the generators, so it reports the words beside the count.
    /// A build whose save format keeps the count alone reports none, and the
    /// two are compared on what they both carry.
    pub(crate) fn positions(&self) -> Positions<V> {
        Positions {
            run: V::run_rng_types()
                .iter()
                .copied()
                .zip(self.run.iter().map(position_of))
                .collect(),
            player: V::player_rng_types()
                .iter()
                .copied()
                .zip(self.player.iter().map(position_of))
                .collect(),
        }
    }
}

/// The run streams a fight draws through, lent as a set.
///
/// `RunState.Rng` holds every stream and any rule in a fight reaches any of
/// these, so they travel together rather than one parameter at a time. Held by
/// reference: the run owns the rows, and a draw moves the row itself.
pub(crate) struct CombatRngs<'a> {
    /// The shuffle stream, because drawing a card can turn the discard pile
    /// back into a draw pile.
    pub(crate) shuffle: &'a mut MegaRng,
    /// The stream monsters use when they roll their hit points.
    pub(crate) niche: &'a mut MegaRng,
    /// The stream a rule that creates or takes cards draws from.
    pub(crate) generation: &'a mut MegaRng,
    /// The stream a monster's move machine rolls against.
    pub(crate) ai: &'a mut MegaRng,
}

impl CombatRngs<'_> {
    /// Lend the set again for a shorter life, which is what handing it to a
    /// nested board needs.
    pub(crate) fn reborrow(&mut self) -> CombatRngs<'_> {
        CombatRngs {
            shuffle: self.shuffle,
            niche: self.niche,
            generation: self.generation,
            ai: self.ai,
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

/// Where a generator stands, in the shape the wire uses.
fn position_of(rng: &MegaRng) -> Position {
    Position {
        counter: u32::try_from(rng.counter()).expect("a stream's draw count is not negative"),
        words: Some(rng.words()),
    }
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
                sts2_core::rng::for_stream::<V0_107_1, MegaRng>(123, kind),
                "{} opened on another stream's seed",
                kind.name()
            );
        }
    }

    /// A draw moves one stream and leaves the rest where they were.
    #[test]
    fn drawing_moves_one_stream() {
        let mut streams = Streams::<V0_107_1>::opening(123);

        for _ in 0..9 {
            streams.run(RunRngType::Shuffle).next_u64();
        }

        let at = streams.positions();
        assert_eq!(at.run[&RunRngType::Shuffle].counter, 9);
        for kind in RunRngType::ALL
            .into_iter()
            .filter(|k| *k != RunRngType::Shuffle)
        {
            assert_eq!(at.run[&kind].counter, 0, "{} moved too", kind.name());
        }
    }

    /// The set a fight draws through lends the rows it names.
    ///
    /// The rows are read by position, so a set built on the wrong positions
    /// hands a fight another stream's generator. Nothing downstream tells the
    /// difference: the draws are real numbers off a real stream, and the wrong
    /// one.
    #[test]
    fn the_combat_set_lends_the_streams_it_names() {
        let named = [
            (RunRngType::Shuffle, 1),
            (RunRngType::Niche, 2),
            (RunRngType::CombatCardGeneration, 3),
            (RunRngType::MonsterAi, 4),
        ];

        let mut streams = Streams::<V0_107_1>::opening(123);
        {
            let rngs = streams.combat();
            let lent = [
                (rngs.shuffle, 1),
                (rngs.niche, 2),
                (rngs.generation, 3),
                (rngs.ai, 4),
            ];
            // A different count each, so a row lent twice or lent in another
            // stream's place shows up as the wrong count rather than as none.
            for (rng, draws) in lent {
                for _ in 0..draws {
                    rng.next_u64();
                }
            }
        }

        let at = streams.positions();
        for (kind, draws) in named {
            assert_eq!(
                at.run[&kind].counter,
                draws,
                "{} was not the row the set lent",
                kind.name()
            );
        }
        for kind in RunRngType::ALL
            .into_iter()
            .filter(|k| !named.iter().any(|(named, _)| named == k))
        {
            assert_eq!(at.run[&kind].counter, 0, "{} moved too", kind.name());
        }
    }

    /// A decision draws from a clone, and the run takes the clone only once
    /// the decision is whole.
    ///
    /// What makes a decision transactional. The streams move where they lie,
    /// so the copy is the whole of the boundary.
    #[test]
    fn a_decision_draws_from_a_clone() {
        let streams = Streams::<V0_107_1>::opening(123);

        let mut taken = streams.clone();
        taken.run(RunRngType::Niche).next_u64();

        assert_eq!(streams.positions().run[&RunRngType::Niche].counter, 0);
        assert_eq!(taken.positions().run[&RunRngType::Niche].counter, 1);
    }
}
