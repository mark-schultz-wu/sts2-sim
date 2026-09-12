//! Replays `megarandom.txt`, dumped from the game, and requires an exact match.
#![allow(clippy::disallowed_macros, clippy::disallowed_types)]
//!
//! Generic over [`GameRng`], so a replacement implementation has to pass the
//! same fixture. Each block reseeds and draws in index order, so index `0`
//! means reseed.

use sts2_core::rng::{Bound, GameRng, Interval, Span};
use sts2_sim::rng::MegaRng;

const FIXTURE: &str = include_str!("rng/megarandom.txt");

#[test]
fn choosing_from_nothing_is_an_invalid_draw() {
    let mut rng = MegaRng::new(0);
    assert_eq!(rng.choose::<u8>(&[]), Err(sts2_core::rng::Error::Bound));
    assert_eq!(rng.counter(), 0);
}

fn num<T: std::str::FromStr>(field: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    field.parse().expect("fixture field should parse")
}

fn bound(field: &str) -> Bound<i32> {
    Bound::new(num(field)).expect("fixture bounds are positive")
}

fn rows(prefix: &str) -> impl Iterator<Item = (usize, Vec<&'static str>)> {
    FIXTURE.lines().enumerate().filter_map(move |(n, line)| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        (fields.first() == Some(&prefix)).then_some((n, fields))
    })
}

fn draws<G: GameRng<Int = i32>>() -> usize {
    let mut rng = G::new(0);
    let mut checked = 0usize;

    for (lineno, f) in FIXTURE.lines().enumerate().filter_map(|(n, line)| {
        let f: Vec<&str> = line.split_whitespace().collect();
        let head = *f.first()?;
        (head != "hash" && head != "forward" && !head.starts_with('#')).then_some((n, f))
    }) {
        let method = f[0];
        let seed: u32 = f[1].parse().unwrap();

        // `int` carries one bound, `range` two, `raw` has no counter column.
        let (index, expected, counter) = match method {
            "int" => (f[3], f[4], Some(f[5])),
            "range" => (f[4], f[5], Some(f[6])),
            "raw" => (f[2], f[3], None),
            _ => (f[2], f[3], Some(f[4])),
        };
        let index: usize = index.parse().unwrap();

        if index == 0 {
            rng = G::new(seed);
        }

        let actual = match method {
            "raw" => format!("{:016x}", rng.next_u64()),
            "bool" => (rng.next_bool() as u8).to_string(),
            "double" => format!("{:016x}", rng.next_double().to_bits()),
            "int" => rng.next_int(bound(f[2])).to_string(),
            "range" => rng
                .next_int_range(Span::new(num(f[2]), num(f[3])).unwrap())
                .to_string(),
            "uint" => rng.next_uint_range(Span::new(3, 99).unwrap()).to_string(),
            "float" => format!(
                "{:08x}",
                rng.next_f32_range(Interval::new(2.0, 5.0)).to_bits()
            ),
            "dblrange" => format!(
                "{:016x}",
                rng.next_double_range(Interval::new(-3.5, 7.25)).to_bits()
            ),
            // Non-degenerate on purpose: with mean 0, stddev 1 and a 0..1 range
            // every scaling step is an identity and the fixture cannot tell
            // `mean + x` from `mean - x`.
            "gauss" => format!(
                "{:016x}",
                rng.next_gaussian_double(0.4, 0.3, Interval::new(10.0, 20.0))
                    .to_bits()
            ),
            // Tight bounds, so the rejection loop actually rejects.
            "gaussint" => rng
                .next_gaussian_int(10, 3, Interval::new(9, 11))
                .to_string(),
            other => panic!("line {lineno}: unknown method {other}"),
        };

        assert_eq!(actual, expected, "line {lineno}: diverged from the game");
        assert_eq!(
            rng.seed(),
            seed,
            "line {lineno}: the stream forgot its seed"
        );

        // A value that matches while the counter drifts is still a divergence:
        // the counter is what streams are rebuilt from.
        if let Some(counter) = counter {
            assert_eq!(
                rng.counter().to_string(),
                counter,
                "line {lineno}: the counter drifted"
            );
        }
        checked += 1;
    }

    checked
}

#[test]
fn every_draw_matches_the_game() {
    // An emptied fixture would pass while proving nothing.
    assert_eq!(
        draws::<MegaRng>(),
        1024,
        "the fixture should carry 1024 draws"
    );
}

/// A stream rebuilt from `(seed, counter)` must equal one drawn to that point.
#[test]
fn fast_forwarding_lands_where_drawing_would_have() {
    let mut checked = 0;
    for (lineno, f) in rows("forward") {
        let (seed, target) = (f[1].parse().unwrap(), f[2].parse().unwrap());
        let mut forwarded = MegaRng::with_counter(seed, target).unwrap();
        assert_eq!(
            forwarded.next_int(bound("1000")).to_string(),
            f[4],
            "line {lineno}"
        );
        assert_eq!(forwarded.counter().to_string(), f[5], "line {lineno}");
        checked += 1;
    }
    assert_eq!(checked, 4);
}

/// Stream identity: a wrong mix lands on a real stream that is the wrong one.
#[test]
fn named_streams_land_on_the_right_seed() {
    let mut checked = 0;
    for (lineno, f) in rows("hash") {
        let expected_seed: u32 = f[2].parse().unwrap();
        let units: Vec<u16> = if f[3] == "-" {
            Vec::new()
        } else {
            f[3].as_bytes()
                .chunks(4)
                .map(|c| u16::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
                .collect()
        };
        let name = String::from_utf16(&units).unwrap();
        assert_eq!(
            MegaRng::named(1000, &name).seed(),
            expected_seed,
            "line {lineno}: {name:?}"
        );
        checked += 1;
    }
    assert_eq!(checked, 15);
}

/// A modulo implementation passes a weaker version of the fixture test, so pin
/// a bound where the two disagree.
#[test]
fn bounded_draws_are_scaled_not_reduced() {
    let mut scaled = MegaRng::new(42);
    let mut reduced = MegaRng::new(42);
    let three = bound("3");
    let differs = (0..64).any(|_| scaled.next_int(three) != (reduced.next_u64() % 3) as i32);
    assert!(differs, "modulo is indistinguishable at this bound");
}

/// Streams are shared, so a method consuming an extra draw shifts everything
/// after it. This is also what makes `fast_forward` able to replay.
#[test]
fn every_method_consumes_exactly_one_draw() {
    let probes: [&dyn Fn(&mut MegaRng); 8] = [
        &|r| {
            r.next_u64();
        },
        &|r| {
            r.next_bool();
        },
        &|r| {
            r.next_int(bound("10"));
        },
        &|r| {
            r.next_int_range(Span::new(-3, 3).unwrap());
        },
        &|r| {
            r.next_uint_range(Span::new(1, 9).unwrap());
        },
        &|r| {
            r.next_f32_range(Interval::new(0.0, 1.0));
        },
        &|r| {
            r.next_double();
        },
        &|r| {
            r.next_double_range(Interval::new(-1.0, 1.0));
        },
    ];

    for (i, probe) in probes.iter().enumerate() {
        let mut rng = MegaRng::new(7);
        probe(&mut rng);
        assert_eq!(rng.counter(), 1, "probe {i} moved the counter wrongly");

        // And the counter has to mean a position, not just a tally.
        assert_eq!(
            rng,
            MegaRng::with_counter(7, 1).unwrap(),
            "probe {i}: the counter and the generator position disagree"
        );
    }
}

/// Seeking backwards is not a slower seek, it is meaningless. It is now a value
/// the caller handles rather than a panic.
#[test]
fn a_stream_refuses_to_rewind() {
    let mut rng = MegaRng::with_counter(7, 10).unwrap();
    assert_eq!(
        rng.fast_forward(3),
        Err(sts2_core::rng::Error::Rewind { from: 10, to: 3 })
    );

    // And the failed seek left it where it was.
    assert_eq!(rng.counter(), 10);
    assert!(rng.fast_forward(10).is_ok(), "seeking to here is a no-op");
}
