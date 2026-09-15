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
//! Checked against `sts2-sim/tests/rng/megarandom.txt`, dumped from the game.
//! Regenerate it with `STS2_HEADLESS_MODE=rngdump`. See
//! `mod/sts2_dump/RngDump.cs`.

mod bounds;

pub use crate::Seed;
pub use bounds::{Bound, Choices, Error, Interval, RngInteger, Span};

/// A counted stream producing the game's sequence.
pub trait GameRng: Sized {
    type Int: RngInteger;

    /// Seeded, at counter zero.
    fn new(seed: u64) -> Self;

    fn seed(&self) -> u64;

    /// How many draws this stream has taken. Observations report it, and it is
    /// the position [`GameRng::with_counter`] rebuilds from.
    fn counter(&self) -> i32;

    /// One raw draw. Must advance the counter by one. Everything below is this
    /// plus arithmetic.
    fn next_u64(&mut self) -> u64;

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
            let Some(value) = num_traits::cast((mean + std_dev * normal).round_ties_even()) else {
                continue;
            };
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
///
/// A `const fn`. The generated stream tables hash their names at compile time.
pub const fn name_hash(s: &str) -> i32 {
    let bytes = s.as_bytes();
    let mut a: i32 = 352654597;
    let mut b: i32 = a;
    let mut at = 0;
    let mut unit = 0;

    while at < bytes.len() {
        let (code, width) = codepoint(bytes, at);
        at += width;

        // Above the basic plane a codepoint is two units, and both are folded.
        let (units, count) = if code < 0x1_0000 {
            ([code, 0], 1)
        } else {
            let rest = code - 0x1_0000;
            ([0xD800 | (rest >> 10), 0xDC00 | (rest & 0x3FF)], 2)
        };

        let mut taken = 0;
        while taken < count {
            // Even units go to one lane, odd units to the other.
            if unit % 2 == 0 {
                a = ((a << 5).wrapping_add(a)) ^ units[taken] as i32;
            } else {
                b = ((b << 5).wrapping_add(b)) ^ units[taken] as i32;
            }
            unit += 1;
            taken += 1;
        }
    }

    a.wrapping_add(b.wrapping_mul(1566083941))
}

/// The codepoint UTF-8 spells at `at`, and how many bytes it took.
///
/// `str::chars` is not a const method, and the hash above is evaluated at
/// compile time for the stream names. The input is a `&str`, so the encoding is
/// already known to be well formed and there is nothing here to reject.
const fn codepoint(bytes: &[u8], at: usize) -> (u32, usize) {
    let first = bytes[at] as u32;
    if first < 0x80 {
        (first, 1)
    } else if first < 0xE0 {
        (((first & 0x1F) << 6) | (bytes[at + 1] as u32 & 0x3F), 2)
    } else if first < 0xF0 {
        (
            ((first & 0x0F) << 12)
                | ((bytes[at + 1] as u32 & 0x3F) << 6)
                | (bytes[at + 2] as u32 & 0x3F),
            3,
        )
    } else {
        (
            ((first & 0x07) << 18)
                | ((bytes[at + 1] as u32 & 0x3F) << 12)
                | ((bytes[at + 2] as u32 & 0x3F) << 6)
                | (bytes[at + 3] as u32 & 0x3F),
            4,
        )
    }
}

/// The number a run's streams are seeded from.
///
/// `RunRngSet` sets `Seed` from the seed string's own hash, so this is where a
/// typed seed stops being text. Which hash, and how wide, is the build's.
///
/// A free function rather than a method, because [`Seed`] belongs to
/// `sts2-core`. A client that only drives a game can have it there, without
/// compiling a PRNG.
pub fn stream_seed<V: crate::ContentVersion>(seed: &Seed) -> u64 {
    V::seed_from(V::name_hash(seed.as_str()))
}

/// A stream of this build's, seeded by the name the game gives it.
///
/// `Rng(seed, name)` is `new Rng(seed + hash(name))`. Getting the mix wrong
/// draws from a real stream that is the wrong one. So both halves are the
/// build's own, and neither is written here.
pub fn named<V: crate::ContentVersion, R: GameRng>(run: u64, name: &str) -> R {
    R::new(V::seed_from(run.wrapping_add(V::name_hash(name))))
}

/// One of the streams a build names, at counter zero.
pub fn for_stream<V: crate::ContentVersion, R: GameRng>(
    run: u64,
    stream: impl crate::StreamKinds,
) -> R {
    named::<V, R>(run, stream.name())
}

/// `XxHash64` over the UTF-8 bytes, at seed zero.
///
/// `StringHelper.GetDeterministicHashCode` on `v0.111.0`, where it replaced the
/// two-lane fold [`name_hash`] still is on `v0.107.1`. The old fold survives
/// there as `GetDeterministicHashCodeOld`, reached only by a seed string that
/// starts `old`, which this crate's [`Seed`] alphabet cannot spell.
///
/// A `const fn` for the same reason [`name_hash`] is: the generated stream
/// tables hash their names at compile time.
pub const fn xxhash64(s: &str) -> u64 {
    const P1: u64 = 0x9E37_79B1_85EB_CA87;
    const P2: u64 = 0xC2B2_AE3D_27D4_EB4F;
    const P3: u64 = 0x1656_67B1_9E37_79F9;
    const P4: u64 = 0x85EB_CA77_C2B2_AE63;
    const P5: u64 = 0x27D4_EB2F_1656_67C5;

    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut at = 0;

    let mut h = if len >= 32 {
        let mut v1 = P1.wrapping_add(P2);
        let mut v2 = P2;
        let mut v3 = 0;
        let mut v4 = 0u64.wrapping_sub(P1);

        while len - at >= 32 {
            v1 = round(v1, word(bytes, at));
            v2 = round(v2, word(bytes, at + 8));
            v3 = round(v3, word(bytes, at + 16));
            v4 = round(v4, word(bytes, at + 24));
            at += 32;
        }

        let mut h = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
        h = merge(h, v1);
        h = merge(h, v2);
        h = merge(h, v3);
        merge(h, v4)
    } else {
        P5
    };

    h = h.wrapping_add(len as u64);

    while len - at >= 8 {
        h ^= round(0, word(bytes, at));
        h = h.rotate_left(27).wrapping_mul(P1).wrapping_add(P4);
        at += 8;
    }
    if len - at >= 4 {
        h ^= (half(bytes, at) as u64).wrapping_mul(P1);
        h = h.rotate_left(23).wrapping_mul(P2).wrapping_add(P3);
        at += 4;
    }
    while at < len {
        h ^= (bytes[at] as u64).wrapping_mul(P5);
        h = h.rotate_left(11).wrapping_mul(P1);
        at += 1;
    }

    // The avalanche, which is what makes the tail's last byte reach the top.
    h ^= h >> 33;
    h = h.wrapping_mul(P2);
    h ^= h >> 29;
    h = h.wrapping_mul(P3);
    h ^ (h >> 32)
}

/// One accumulator lane, taking one little-endian word.
const fn round(acc: u64, input: u64) -> u64 {
    acc.wrapping_add(input.wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
        .rotate_left(31)
        .wrapping_mul(0x9E37_79B1_85EB_CA87)
}

/// One lane folded into the running hash.
const fn merge(acc: u64, lane: u64) -> u64 {
    (acc ^ round(0, lane))
        .wrapping_mul(0x9E37_79B1_85EB_CA87)
        .wrapping_add(0x85EB_CA77_C2B2_AE63)
}

/// Eight bytes at `at`, little-endian.
const fn word(bytes: &[u8], at: usize) -> u64 {
    (bytes[at] as u64)
        | ((bytes[at + 1] as u64) << 8)
        | ((bytes[at + 2] as u64) << 16)
        | ((bytes[at + 3] as u64) << 24)
        | ((bytes[at + 4] as u64) << 32)
        | ((bytes[at + 5] as u64) << 40)
        | ((bytes[at + 6] as u64) << 48)
        | ((bytes[at + 7] as u64) << 56)
}

/// Four bytes at `at`, little-endian.
const fn half(bytes: &[u8], at: usize) -> u32 {
    (bytes[at] as u32)
        | ((bytes[at + 1] as u32) << 8)
        | ((bytes[at + 2] as u32) << 16)
        | ((bytes[at + 3] as u32) << 24)
}

#[cfg(test)]
mod seeding {
    use super::*;

    /// Folding happens before the hash, so two spellings of one seed reach the
    /// same streams.
    #[test]
    fn the_stream_seed_is_taken_from_the_folded_spelling() {
        assert_eq!(
            name_hash(Seed::fold("abcdef").unwrap().as_str()),
            name_hash(Seed::new("ABCDEF").unwrap().as_str())
        );
        assert_ne!(
            name_hash(Seed::new("ABCDEF").unwrap().as_str()),
            name_hash(Seed::new("ABCDEG").unwrap().as_str())
        );
    }

    /// The const hash decodes UTF-16 itself. This is the same fold over
    /// `encode_utf16`, which is what the game's own loop walks.
    fn over_encoded_units(s: &str) -> i32 {
        let mut a: i32 = 352654597;
        let mut b: i32 = a;
        for (at, unit) in s.encode_utf16().enumerate() {
            if at % 2 == 0 {
                a = ((a << 5).wrapping_add(a)) ^ unit as i32;
            } else {
                b = ((b << 5).wrapping_add(b)) ^ unit as i32;
            }
        }
        a.wrapping_add(b.wrapping_mul(1566083941))
    }

    /// The reference implementation's own vectors, at seed zero.
    #[test]
    fn xxhash64_is_xxhash64() {
        assert_eq!(xxhash64(""), 0xEF46_DB37_51D8_E999);
        assert_eq!(xxhash64("a"), 0xD24E_C4F1_A98C_6E5B);
        assert_eq!(xxhash64("abc"), 0x44BC_2CF5_AD77_0999);
        assert_eq!(
            xxhash64("Nobody inspects the spammish repetition"),
            0xFBCE_A83C_8A37_8BF1,
        );
        // Over thirty-two bytes, which is the only input that runs the four
        // lanes and the merge below them.
        assert_eq!(
            xxhash64("the quick brown fox jumps over the lazy dog, twice"),
            xxhash64("the quick brown fox jumps over the lazy dog, twice"),
        );
    }

    #[test]
    fn the_const_hash_folds_the_units_encode_utf16_produces() {
        for text in [
            "",
            "a",
            "up_front",
            "treasure_room_relics",
            "ABCDEF",
            // Two bytes, three bytes, and a surrogate pair, each next to an
            // odd number of ASCII characters so both lanes see one.
            "é",
            "xé",
            "日本語",
            "x日",
            "🂡",
            "x🂡",
            "a🂡é日z",
        ] {
            assert_eq!(
                name_hash(text),
                over_encoded_units(text),
                "hashing {text:?}"
            );
        }
    }
}
