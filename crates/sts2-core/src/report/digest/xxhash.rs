//! xxHash64, seed zero.
//!
//! Written out rather than depended on, the way `sts2-rng` writes out the
//! game's generator. The digest's value is part of this crate's definition, and
//! a dependency is not allowed to move it. A published algorithm rather than
//! something invented here, so the test below can hold it against the reference
//! vectors and say which one it is.
//!
//! Streaming, because the encoding is written straight in and never exists as a
//! buffer to hash in one go.
pub(super) struct XxHash64 {
    /// Bytes seen, which decides which of the two finishing paths runs.
    total: u64,
    lanes: [u64; 4],
    /// The bytes since the last full stripe.
    held: [u8; 32],
    /// How many of `held` are real.
    filled: usize,
}

const P1: u64 = 0x9E37_79B1_85EB_CA87;
const P2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const P3: u64 = 0x1656_67B1_9E37_79F9;
const P4: u64 = 0x85EB_CA77_C2B2_AE63;
const P5: u64 = 0x27D4_EB2F_1656_67C5;

impl XxHash64 {
    pub(super) fn new() -> XxHash64 {
        XxHash64 {
            total: 0,
            lanes: [P1.wrapping_add(P2), P2, 0, 0u64.wrapping_sub(P1)],
            held: [0; 32],
            filled: 0,
        }
    }

    pub(super) fn write(&mut self, mut bytes: &[u8]) {
        self.total += bytes.len() as u64;

        if self.filled > 0 {
            let wanted = 32 - self.filled;
            if bytes.len() < wanted {
                self.held[self.filled..self.filled + bytes.len()].copy_from_slice(bytes);
                self.filled += bytes.len();
                return;
            }

            self.held[self.filled..].copy_from_slice(&bytes[..wanted]);
            bytes = &bytes[wanted..];
            let stripe = self.held;
            self.stripe(&stripe);
            self.filled = 0;
        }

        while bytes.len() >= 32 {
            let (stripe, rest) = bytes.split_at(32);
            self.stripe(stripe);
            bytes = rest;
        }

        self.held[..bytes.len()].copy_from_slice(bytes);
        self.filled = bytes.len();
    }

    fn stripe(&mut self, stripe: &[u8]) {
        for (lane, chunk) in self.lanes.iter_mut().zip(stripe.chunks_exact(8)) {
            *lane = round(*lane, word(chunk));
        }
    }

    pub(super) fn finish(&self) -> u64 {
        let mut acc = if self.total >= 32 {
            let [a, b, c, d] = self.lanes;
            let mut acc = a
                .rotate_left(1)
                .wrapping_add(b.rotate_left(7))
                .wrapping_add(c.rotate_left(12))
                .wrapping_add(d.rotate_left(18));
            for lane in [a, b, c, d] {
                acc = (acc ^ round(0, lane)).wrapping_mul(P1).wrapping_add(P4);
            }
            acc
        } else {
            P5
        };

        acc = acc.wrapping_add(self.total);

        let mut rest = &self.held[..self.filled];
        while rest.len() >= 8 {
            acc = (acc ^ round(0, word(&rest[..8])))
                .rotate_left(27)
                .wrapping_mul(P1)
                .wrapping_add(P4);
            rest = &rest[8..];
        }
        if rest.len() >= 4 {
            let half = u32::from_le_bytes(rest[..4].try_into().expect("four bytes"));
            acc = (acc ^ u64::from(half).wrapping_mul(P1))
                .rotate_left(23)
                .wrapping_mul(P2)
                .wrapping_add(P3);
            rest = &rest[4..];
        }
        for &byte in rest {
            acc = (acc ^ u64::from(byte).wrapping_mul(P5))
                .rotate_left(11)
                .wrapping_mul(P1);
        }

        acc ^= acc >> 33;
        acc = acc.wrapping_mul(P2);
        acc ^= acc >> 29;
        acc = acc.wrapping_mul(P3);
        acc ^ (acc >> 32)
    }
}

fn round(lane: u64, input: u64) -> u64 {
    lane.wrapping_add(input.wrapping_mul(P2))
        .rotate_left(31)
        .wrapping_mul(P1)
}

fn word(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("eight bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(bytes: &[u8]) -> u64 {
        let mut h = XxHash64::new();
        h.write(bytes);
        h.finish()
    }

    /// It is xxHash64 and not something shaped like it.
    ///
    /// The reference vectors, seed zero. Written out rather than depended on
    /// means the algorithm is this crate's to get wrong, and these are what say
    /// it did not. The four lengths straddle the 32-byte stripe, which is the
    /// only interesting boundary in the algorithm.
    #[test]
    fn the_hash_is_xxhash64() {
        assert_eq!(hash(b""), 0xef46_db37_51d8_e999);
        assert_eq!(hash(b"a"), 0xd24e_c4f1_a98c_6e5b);
        assert_eq!(hash(b"abc"), 0x44bc_2cf5_ad77_0999);
        assert_eq!(
            hash(b"Nobody inspects the spammish repetition"),
            0xfbce_a83c_8a37_8bf1
        );

        assert_eq!(hash(&[b'x'; 31]), 0x60dd_0d01_083b_99f0);
        assert_eq!(hash(&[b'x'; 32]), 0xe2df_261f_c2ec_30eb);
        assert_eq!(hash(&[b'x'; 33]), 0xb3fa_465f_5542_08a6);

        let every_byte: Vec<u8> = (0..=255).collect();
        assert_eq!(hash(&every_byte), 0x1fac_be84_06cd_904b);
    }

    /// The same bytes hash the same however they arrive.
    ///
    /// The encoding is written in a field at a time. The hash sees a stream of
    /// two-, four- and eight-byte writes rather than one buffer, and the stripe
    /// buffer is what makes that not matter. The vectors above only ever
    /// exercise it in one chunk.
    #[test]
    fn the_chunking_does_not_reach_the_hash() {
        let bytes: Vec<u8> = (0..200u32).map(|i| (i * 37 % 251) as u8).collect();
        let whole = hash(&bytes);

        for chunk in 1..=40 {
            let mut h = XxHash64::new();
            for part in bytes.chunks(chunk) {
                h.write(part);
            }
            assert_eq!(h.finish(), whole, "in chunks of {chunk}");
        }
    }
}
