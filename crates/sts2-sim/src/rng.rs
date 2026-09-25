//! The game's concrete random stream.

use sts2_core::rng::GameRng;

/// xoshiro256** seeded through splitmix64, counted.
///
/// `Copy`, so a row of them copies rather than walking. Nothing here holds one
/// by value. A run keeps them in [`crate::stream::Streams`], which hands out
/// `&mut`, so a copy is a thing the author writes down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MegaRng {
    s: [u64; 4],
    counter: i32,
}

impl MegaRng {
    /// The generator's own four words, which is what the game saves beside the
    /// draw count.
    pub fn words(&self) -> [u64; 4] {
        self.s
    }

    /// A generator standing where these words and this count leave it, which
    /// is how the game restores a saved stream.
    pub fn from_words(words: [u64; 4], counter: i32) -> MegaRng {
        MegaRng { s: words, counter }
    }
}

impl GameRng for MegaRng {
    type Int = i32;

    fn new(seed: u64) -> Self {
        let mut x = seed;
        Self {
            s: [
                splitmix64(&mut x),
                splitmix64(&mut x),
                splitmix64(&mut x),
                splitmix64(&mut x),
            ],
            counter: 0,
        }
    }

    fn counter(&self) -> i32 {
        self.counter
    }

    fn next_u64(&mut self) -> u64 {
        self.counter += 1;
        let [s0, s1, s2, s3] = self.s;
        let result = (s1.wrapping_mul(5)).rotate_left(7).wrapping_mul(9);
        let t = s1 << 17;
        let s2 = s2 ^ s0;
        let s3 = s3 ^ s1;
        let s1 = s1 ^ s2;
        let s0 = s0 ^ s3;
        let s2 = s2 ^ t;
        self.s = [s0, s1, s2, s3.rotate_left(45)];
        result
    }
}

fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
