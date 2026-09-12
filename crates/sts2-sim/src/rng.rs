//! The game's concrete random stream.

use sts2_core::rng::GameRng;

/// xoshiro256** seeded through splitmix64, counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MegaRng {
    s: [u64; 4],
    seed: u32,
    counter: i32,
}

impl GameRng for MegaRng {
    type Int = i32;

    fn new(seed: u32) -> Self {
        let mut x = u64::from(seed);
        Self {
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

    fn seed(&self) -> u32 {
        self.seed
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
