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
//! Checked against `tests/rng/<build>.txt`, dumped from the game. Regenerate
//! with `STS2_HEADLESS_MODE=rngdump`. See `mod/sts2_dump/RngDump.cs`.
//!
//! The draw stream is one implementation for every build this repository pins.
//! What a build changes is [`Seeding`]: how a name becomes the number a stream
//! starts from, and how wide that number is.

mod bounds;
mod hash;

pub use bounds::{Bound, Error, Interval, Span};
pub use hash::{interleaved, xxhash64};
pub use sts2_core::Seed;

/// `next_bool` draws over two outcomes. Checked at compile time.
const TWO: Bound = match Bound::new(2) {
    Some(bound) => bound,
    None => unreachable!(),
};

/// A counted stream producing the game's sequence.
pub trait GameRng: Sized {
    /// Seeded, at counter zero.
    ///
    /// A `u64`, because v0.111.0 widened a stream seed from `uint`. A narrower
    /// seed reaches the same state through it: the game's own `uint`
    /// constructor widens before seeding, and so does this.
    fn new(seed: u64) -> Self;

    fn seed(&self) -> u64;

    /// How many draws this stream has taken. Observations report it, and it is
    /// the position [`GameRng::with_counter`] rebuilds from.
    fn counter(&self) -> i32;

    /// One raw draw. Must advance the counter by one. Everything below is this
    /// plus arithmetic.
    fn next_u64(&mut self) -> u64;

    /// Seeded by mixing in a name, the way the build names its streams.
    ///
    /// The mix belongs to [`Seeding`], because it is the part a build changes.
    /// Getting it wrong draws from a real stream that is the wrong one.
    fn named<S: Seeding>(seed: S::Seed, name: &str) -> Self {
        Self::new(S::named(seed, name).into())
    }

    /// A stream already advanced to `counter`, equal to one drawn there.
    fn with_counter(seed: u64, counter: i32) -> Result<Self, Error> {
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
        self.next_int(TWO) == 0
    }

    /// `[0, max)`, by scaling a double and truncating.
    #[inline]
    fn next_int(&mut self, max: Bound) -> i32 {
        (self.next_double() * max.get() as f64) as i32
    }

    /// A span wider than `i32::MAX` takes the game's 64-bit branch, which is a
    /// different computation and not just a wider one.
    #[inline]
    fn next_int_range(&mut self, range: Span<i32>) -> i32 {
        let width = range.max() as i64 - range.min() as i64;
        if width <= i32::MAX as i64 {
            ((self.next_double() * width as f64) as i32).wrapping_add(range.min())
        } else {
            (((self.next_double() * width as f64) as i64).wrapping_add(range.min() as i64)) as i32
        }
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

    /// Box–Muller with rejection: two draws per attempt, rejecting against
    /// `[0, 1]` regardless of `scale`, because the scaling happens after. So how
    /// far the counter moves is part of what has to be reproduced.
    fn next_gaussian_double(&mut self, mean: f64, std_dev: f64, scale: Interval<f64>) -> f64 {
        loop {
            let a = 1.0 - self.next_double();
            let b = 1.0 - self.next_double();
            let value =
                mean + (-2.0 * a.ln()).sqrt() * (core::f64::consts::PI * 2.0 * b).cos() * std_dev;
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
    fn next_gaussian_int(&mut self, mean: i32, std_dev: i32, accept: Interval<i32>) -> i32 {
        loop {
            let a = 1.0 - self.next_double();
            let b = 1.0 - self.next_double();
            let normal = (-2.0 * a.ln()).sqrt() * (core::f64::consts::PI * 2.0 * b).sin();
            let value = (mean as f64 + std_dev as f64 * normal).round_ties_even() as i32;
            if value >= accept.min() && value <= accept.max() {
                return value;
            }
        }
    }
}

/// `(u64 >> 11) * this` is the game's `NextDouble`. 2⁻⁵³.
const INCR_DOUBLE: f64 = 1.110_223_024_625_156_5e-16;

/// xoshiro256** seeded through splitmix64, counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MegaRng {
    s: [u64; 4],
    seed: u64,
    counter: i32,
}

impl GameRng for MegaRng {
    fn new(seed: u64) -> Self {
        let mut x = seed;
        MegaRng {
            s: [
                splitmix64(&mut x),
                splitmix64(&mut x),
                splitmix64(&mut x),
                splitmix64(&mut x),
            ],
            seed,
            counter: 0,
        }
    }

    fn seed(&self) -> u64 {
        self.seed
    }

    fn counter(&self) -> i32 {
        self.counter
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.counter += 1;
        let [s0, s1, s2, s3] = self.s;

        // Scrambled from s1 *before* the state advances, not after.
        let result = (s1.wrapping_mul(5)).rotate_left(7).wrapping_mul(9);

        let t = s1 << 17;
        let s2 = s2 ^ s0;
        let s3 = s3 ^ s1;
        let s1 = s1 ^ s2;
        let s0 = s0 ^ s3;
        let s2 = s2 ^ t;
        let s3 = s3.rotate_left(45);

        self.s = [s0, s1, s2, s3];
        result
    }
}

/// The seeding mixer. Advances `x`, which is how four state words come out of
/// one seed.
fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// How a build turns text into the number a stream starts from.
///
/// The draw stream is the same on every build this repository pins. This is
/// what a build changes. It changes two things at once: which hash names a
/// stream, and how wide the resulting seed is.
///
/// Width is not a detail. On [`Interleaved`] the sum wraps at 2³², and on
/// [`XxHash`] it does not.
pub trait Seeding {
    /// A stream seed on this build.
    ///
    /// `From<u32>` because the game seeds some streams from a small literal,
    /// and `Into<u64>` because the widest of them holds every other.
    type Seed: Copy + std::fmt::Debug + Eq + From<u32> + Into<u64>;

    /// The hash a name goes through, as this build's seed.
    fn hash(name: &str) -> Self::Seed;

    /// A named stream's seed: the run's number, plus the name's.
    fn named(seed: Self::Seed, name: &str) -> Self::Seed;

    /// The number a run's streams are seeded from.
    ///
    /// `RunRngSet` takes it from the seed the player typed, so this is where a
    /// typed seed stops being text.
    fn stream_seed(seed: &Seed) -> Self::Seed;
}

/// v0.107.1 and earlier: the two-lane hash, and `uint` seeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interleaved;

impl Seeding for Interleaved {
    type Seed = u32;

    fn hash(name: &str) -> u32 {
        interleaved(name) as u32
    }

    fn named(seed: u32, name: &str) -> u32 {
        seed.wrapping_add(Self::hash(name))
    }

    fn stream_seed(seed: &Seed) -> u32 {
        Self::hash(seed.as_str())
    }
}

/// v0.111.0 and later: XXH64, and `ulong` seeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XxHash;

impl Seeding for XxHash {
    type Seed = u64;

    fn hash(name: &str) -> u64 {
        xxhash64(name.as_bytes())
    }

    fn named(seed: u64, name: &str) -> u64 {
        seed.wrapping_add(Self::hash(name))
    }

    fn stream_seed(seed: &Seed) -> u64 {
        Self::hash(seed.as_str())
    }
}

#[cfg(test)]
mod seeding {
    use super::*;

    /// Folding happens before the hash, so two spellings of one seed reach the
    /// same streams. True of every build, because folding is not the hash.
    #[test]
    fn the_stream_seed_is_taken_from_the_folded_spelling() {
        fn folds_the_same<S: Seeding>() {
            assert_eq!(
                S::stream_seed(&Seed::fold("abcdef").unwrap()),
                S::stream_seed(&Seed::new("ABCDEF").unwrap())
            );
            assert_ne!(
                S::stream_seed(&Seed::new("ABCDEF").unwrap()),
                S::stream_seed(&Seed::new("ABCDEG").unwrap())
            );
        }

        folds_the_same::<Interleaved>();
        folds_the_same::<XxHash>();
    }

    /// The two builds do not agree, which is the whole reason both are here.
    #[test]
    fn the_builds_seed_a_run_differently() {
        let seed = Seed::new("ABCDEF").unwrap();
        assert_ne!(
            u64::from(Interleaved::stream_seed(&seed)),
            XxHash::stream_seed(&seed)
        );
    }
}
