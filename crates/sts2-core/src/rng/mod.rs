//! The game's randomness: a trait, and one implementation of it.
//!
//! [`GameRng`] is a counted draw stream, the port of
//! `MegaCrit.Sts2.Core.Random.Rng`. An implementation supplies seeding and one
//! raw draw. Every gameplay method defaults to the game's own arithmetic, so a
//! replacement can only get the core wrong, not the semantics.
//!
//! Two things bite anyone porting this:
//!
//! * Bounded draws scale a double and truncate: `(int)(NextDouble() * max)`.
//!   Not a modulo, not rejection sampling. Biased in a way the game depends on.
//! * [`GameRng::next_bool`] is `next_int(2) == 0`, not a bit test.
//!
//! Every method costs exactly one raw draw and advances [`GameRng::counter`], so
//! [`GameRng::with_counter`] can rebuild a stream from where it left off. An
//! extra draw anywhere shifts everything after it.
//!
//! The game guards these methods by throwing. Here the guards are in the
//! argument types: [`Bound`], [`Span`], [`Interval`]. A draw that is reachable
//! is a draw that is valid, and the only fallible operations are building an
//! argument and seeking a stream forward.
//!
//! Checked against `tests/megarandom.txt`, dumped from the game. Regenerate
//! with `STS2_HEADLESS_MODE=rngdump`. See `mod/sts2_dump/RngDump.cs`.

mod bounds;

pub use crate::Seed;
pub use bounds::{Bound, Choices, Error, Interval, RngInteger, Span};

/// A counted stream producing the game's sequence.
pub trait GameRng: Sized {
    type Int: RngInteger;

    /// Seeded, at counter zero.
    fn new(seed: u32) -> Self;

    fn seed(&self) -> u32;

    /// How many draws this stream has taken. Observations report it, and it is
    /// the position [`GameRng::with_counter`] rebuilds from.
    fn counter(&self) -> i32;

    /// One raw draw. Must advance the counter by one. Everything below is this
    /// plus arithmetic.
    fn next_u64(&mut self) -> u64;

    /// Seeded by mixing in a name, the way the game names its streams. Getting
    /// the mix wrong draws from a real stream that is the wrong one.
    fn named(seed: u32, name: &str) -> Self {
        Self::new(seed.wrapping_add(name_hash(name) as u32))
    }

    /// A stream already advanced to `counter`, equal to one drawn there.
    fn with_counter(seed: u32, counter: i32) -> Result<Self, Error> {
        let mut stream = Self::new(seed);
        stream.fast_forward(counter)?;
        Ok(stream)
    }

    /// Replays forward, one draw at a time.
    ///
    /// Fails rather than panics on going backwards. A counter read from an
    /// observation is input, and input that disagrees with where this stream
    /// already is belongs to the caller to handle.
    fn fast_forward(&mut self, target: i32) -> Result<(), Error> {
        if self.counter() > target {
            return Err(Error::Rewind {
                from: self.counter(),
                to: target,
            });
        }
        while self.counter() < target {
            self.next_u64();
        }
        Ok(())
    }

    /// 53 significant bits, in `[0, 1)`.
    #[inline]
    fn next_double(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * INCR_DOUBLE
    }

    /// `next_int(2) == 0`: a scaled double against zero, not a bit test.
    #[inline]
    fn next_bool(&mut self) -> bool {
        self.next_int(
            Bound::try_new(Self::Int::ONE + Self::Int::ONE)
                .expect("one plus one is a positive RNG bound"),
        ) == Self::Int::ZERO
    }

    /// `[0, max)`, by scaling a double and truncating.
    #[inline]
    fn next_int(&mut self, max: Bound<Self::Int>) -> Self::Int {
        Self::Int::scaled(self.next_double(), max.get())
    }

    /// One item selected by the game's bounded integer draw.
    ///
    /// An empty slice or one too large for the RNG's integer bound is
    /// an invalid draw. A successful selection consumes one draw.
    fn choose<'a, T>(&mut self, items: &'a [T]) -> Result<&'a T, Error> {
        Choices::<T, Self::Int>::try_new(items).map(|items| self.choose_from(items))
    }

    /// One item from a slice already known to be a valid choice list.
    fn choose_from<'a, T>(&mut self, choices: Choices<'a, T, Self::Int>) -> &'a T {
        let at: usize = num_traits::cast(self.next_int(choices.bound()))
            .expect("a bounded draw is a valid slice index");
        &choices.items()[at]
    }

    /// One integer from a closed interval in an RNG integer representation.
    fn next_interval<I: RngInteger>(&mut self, range: Interval<I>) -> I {
        I::in_half_open(self.next_double(), range.min(), range.max() + I::ONE)
    }

    /// A wide span can take a different computation from an ordinary span.
    #[inline]
    fn next_int_range(&mut self, range: Span<Self::Int>) -> Self::Int {
        Self::Int::in_half_open(self.next_double(), range.min(), range.max())
    }

    /// The width is computed in unsigned arithmetic before scaling, so this is
    /// not the signed path with a cast.
    #[inline]
    fn next_uint_range(&mut self, range: Span<u32>) -> u32 {
        let width = range.max().wrapping_sub(range.min()) as f64;
        range
            .min()
            .wrapping_add((self.next_double() * width) as u32)
    }

    /// The width is narrowed to `f32` *before* widening back to `f64`, so this
    /// is not [`GameRng::next_double_range`] with a cast.
    #[inline]
    fn next_f32_range(&mut self, range: Interval<f32>) -> f32 {
        (self.next_double() * (range.max() - range.min()) as f64 + range.min() as f64) as f32
    }

    #[inline]
    fn next_double_range(&mut self, range: Interval<f64>) -> f64 {
        self.next_double() * (range.max() - range.min()) + range.min()
    }

    /// Fisher-Yates from the top, the way the game shuffles.
    ///
    /// One draw per element after the first, taken even when the swap is with
    /// itself. Skipping those leaves the stream in the wrong place for
    /// everything after it.
    ///
    /// Unstable: the result depends on the order handed in.
    fn unstable_shuffle<T>(&mut self, items: &mut [T]) {
        let mut at = items.len();
        while at > 1 {
            at -= 1;
            let Some(count) = num_traits::cast(at + 1) else {
                panic!("a shuffle is too large for the RNG's integer bound")
            };
            let Ok(bound) = Bound::try_new(count) else {
                // `at` is at least one here, so the bound is at least two.
                unreachable!("a bound of {} is not zero", at + 1)
            };
            let with = num_traits::cast(self.next_int(bound))
                .expect("a shuffle draw is a valid slice index");
            items.swap(at, with);
        }
    }

    /// The same, over a list put in order first.
    ///
    /// The order handed in then cannot matter. The game shuffles sets whose
    /// order is an accident of how they were collected, and two players must
    /// still get the same answer.
    fn stable_shuffle<T: Ord>(&mut self, items: &mut [T]) {
        items.sort();
        self.unstable_shuffle(items);
    }

    /// The same, over a list whose order is a key rather than the item.
    ///
    /// `List.Sort` goes through `IComparable`, and what a card compares on is
    /// two of its fields rather than the whole of it. The game's own sort is
    /// unstable and this one is not. That shows only on items the key cannot
    /// tell apart, which the game cannot tell apart either.
    fn stable_shuffle_by_key<T, K: Ord>(&mut self, items: &mut [T], key: impl FnMut(&T) -> K) {
        items.sort_by_key(key);
        self.unstable_shuffle(items);
    }

    /// Box–Muller with rejection: two draws per attempt, rejecting against
    /// `[0, 1]` regardless of `scale`, because the scaling happens after. So how
    /// far the counter moves is part of what has to be reproduced.
    fn next_gaussian_double(&mut self, mean: f64, std_dev: f64, scale: Interval<f64>) -> f64 {
        loop {
            let a = 1.0 - self.next_double();
            let b = 1.0 - self.next_double();
            let value = mean + (-2.0 * libm::log(a)).sqrt() * libm::cos(TAU * b) * std_dev;
            if (0.0..=1.0).contains(&value) {
                return value * (scale.max() - scale.min()) + scale.min();
            }
        }
    }

    /// As above but with `sin`, and rejecting against `accept` directly.
    ///
    /// Rounds half to even, which is C#'s `Math.Round` default. Rust's
    /// `f64::round` goes half away from zero and disagrees on exact ties.
    ///
    /// The game has no guard here at all: an inverted interval spins forever
    /// rather than throwing. [`Interval`] is what makes that unreachable.
    fn next_gaussian_int(
        &mut self,
        mean: Self::Int,
        std_dev: Self::Int,
        accept: Interval<Self::Int>,
    ) -> Self::Int {
        loop {
            let a = 1.0 - self.next_double();
            let b = 1.0 - self.next_double();
            let normal = (-2.0 * libm::log(a)).sqrt() * libm::sin(TAU * b);
            let mean = num_traits::cast::<_, f64>(mean).expect("an RNG integer converts to f64");
            let std_dev =
                num_traits::cast::<_, f64>(std_dev).expect("an RNG integer converts to f64");
            let value = Self::Int::from_f64((mean + std_dev * normal).round_ties_even());
            if value >= accept.min() && value <= accept.max() {
                return value;
            }
        }
    }
}

/// A whole turn, which is what Box-Muller takes the angle over.
const TAU: f64 = core::f64::consts::PI * 2.0;

/// `(u64 >> 11) * this` is the game's `NextDouble`. 2⁻⁵³.
const INCR_DOUBLE: f64 = 1.110_223_024_625_156_5e-16;

/// `StringHelper.GetDeterministicHashCode`: two interleaved lanes over UTF-16
/// code units, in wrapping `i32`.
///
/// Public because a stream is not always named through [`GameRng::named`]. An
/// event mixes its own id in beside the player's slot. That sum is the seed, so
/// a caller needs the number and not a stream built from it.
pub fn name_hash(s: &str) -> i32 {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut a: i32 = 352654597;
    let mut b: i32 = a;

    let mut i = 0;
    while i < units.len() {
        a = ((a << 5).wrapping_add(a)) ^ units[i] as i32;
        if i == units.len() - 1 {
            break;
        }
        b = ((b << 5).wrapping_add(b)) ^ units[i + 1] as i32;
        i += 2;
    }

    a.wrapping_add(b.wrapping_mul(1566083941))
}

/// The number a run's streams are seeded from.
///
/// `RunRngSet` is `Seed = (uint)StringHelper.GetDeterministicHashCode(seed)`, so
/// this is where a typed seed stops being text. A free function rather than a
/// method, because [`Seed`] belongs to `sts2-core`. A client that only drives a
/// game can have it there, without compiling a PRNG to get it.
pub fn stream_seed(seed: &Seed) -> u32 {
    name_hash(seed.as_str()) as u32
}

#[cfg(test)]
mod seeding {
    use super::*;

    /// Folding happens before the hash, so two spellings of one seed reach the
    /// same streams.
    #[test]
    fn the_stream_seed_is_taken_from_the_folded_spelling() {
        assert_eq!(
            stream_seed(&Seed::fold("abcdef").unwrap()),
            stream_seed(&Seed::new("ABCDEF").unwrap())
        );
        assert_ne!(
            stream_seed(&Seed::new("ABCDEF").unwrap()),
            stream_seed(&Seed::new("ABCDEG").unwrap())
        );
    }
}
