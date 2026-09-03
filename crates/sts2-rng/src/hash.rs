//! Turning a name into a number, the two ways the game has done it.
//!
//! Both are `StringHelper.GetDeterministicHashCode`. The name did not change
//! between builds and the algorithm did. So they are told apart here by what
//! they compute, rather than by what the game calls them.
//!
//! A stream is identified by the number that comes out of here. Getting one
//! wrong does not produce noise. It draws from a real stream that is the wrong
//! one, which looks like a rule being modelled wrong.

/// The two-lane hash, over UTF-16 code units, in wrapping `i32`.
///
/// v0.107.1 and every build before it. v0.111.0 keeps it as
/// `GetDeterministicHashCodeOld` and stops calling it.
pub fn interleaved(s: &str) -> i32 {
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

/// XXH64 over UTF-8 bytes, seeded with zero.
///
/// v0.111.0 and later, through `System.IO.Hashing.XxHash64.HashToUInt64`. The
/// switch also widened a stream seed to `ulong`, so the arithmetic that adds
/// one to a seed stops wrapping at 2³².
///
/// Ported rather than taken from a crate, on the same terms as `MegaRng`. This
/// has to be the game's numbers, and a dependency is a second thing to check.
pub fn xxhash64(bytes: &[u8]) -> u64 {
    const P1: u64 = 0x9E37_79B1_85EB_CA87;
    const P2: u64 = 0xC2B2_AE3D_27D4_EB4F;
    const P3: u64 = 0x1656_67B1_9E37_79F9;
    const P4: u64 = 0x85EB_CA77_C2B2_AE63;
    const P5: u64 = 0x27D4_EB2F_1656_67C5;

    fn word(bytes: &[u8]) -> u64 {
        u64::from_le_bytes(bytes[..8].try_into().expect("eight bytes"))
    }

    fn round(acc: u64, input: u64) -> u64 {
        acc.wrapping_add(input.wrapping_mul(P2))
            .rotate_left(31)
            .wrapping_mul(P1)
    }

    fn merge(acc: u64, lane: u64) -> u64 {
        (acc ^ round(0, lane)).wrapping_mul(P1).wrapping_add(P4)
    }

    let mut rest = bytes;
    let mut acc = if bytes.len() >= 32 {
        // Four lanes, each a stripe of the input, until fewer than 32 bytes
        // are left. The seed is zero, so the offsets are the primes alone.
        let mut lanes = [P1.wrapping_add(P2), P2, 0, 0u64.wrapping_sub(P1)];
        while rest.len() >= 32 {
            for (n, lane) in lanes.iter_mut().enumerate() {
                *lane = round(*lane, word(&rest[n * 8..]));
            }
            rest = &rest[32..];
        }

        let mut acc = lanes[0]
            .rotate_left(1)
            .wrapping_add(lanes[1].rotate_left(7))
            .wrapping_add(lanes[2].rotate_left(12))
            .wrapping_add(lanes[3].rotate_left(18));
        for lane in lanes {
            acc = merge(acc, lane);
        }
        acc
    } else {
        P5
    };

    // The length, then the tail in narrowing steps: eight bytes, four, one.
    acc = acc.wrapping_add(bytes.len() as u64);

    while rest.len() >= 8 {
        acc = (acc ^ round(0, word(rest)))
            .rotate_left(27)
            .wrapping_mul(P1)
            .wrapping_add(P4);
        rest = &rest[8..];
    }

    if rest.len() >= 4 {
        let half = u32::from_le_bytes(rest[..4].try_into().expect("four bytes")) as u64;
        acc = (acc ^ half.wrapping_mul(P1))
            .rotate_left(23)
            .wrapping_mul(P2)
            .wrapping_add(P3);
        rest = &rest[4..];
    }

    for byte in rest {
        acc = (acc ^ (*byte as u64).wrapping_mul(P5))
            .rotate_left(11)
            .wrapping_mul(P1);
    }

    acc ^= acc >> 33;
    acc = acc.wrapping_mul(P2);
    acc ^= acc >> 29;
    acc = acc.wrapping_mul(P3);
    acc ^ (acc >> 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published XXH64 vector for the empty input at seed zero.
    ///
    /// Here as well as in the fixture. It is the one value that says this is
    /// XXH64, rather than something that agrees with the game on fifteen
    /// strings.
    #[test]
    fn the_empty_string_hashes_to_the_published_vector() {
        assert_eq!(xxhash64(b""), 0xEF46_DB37_51D8_E999);
    }
}
