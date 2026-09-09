//! The blessing a run opens on, against the one the game rolled.
//!
//! `tests/golden/<run>/room` is what the oracle reported at the first step,
//! written by `cargo xtask golden`. Neow rolls its three options off the
//! event's own stream, so no counter in an observation says whether a
//! simulator rolled them the same way. Only the relics do.

use std::path::Path;

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::Seed;
use sts2_core::rng::{MegaRng, stream_seed};
use sts2_sim::event::Blessing;

type V = V0_107_1;

const RUN: &str = "0RACLE1-REGENT-2";
const SEED: &str = "0RACLE1";

/// The relics the recorded room offered, in the order it offered them.
fn offered() -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(RUN)
        .join("room");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));

    text.lines()
        .filter(|line| line.starts_with("option "))
        .map(|line| {
            let key = line
                .split_whitespace()
                .nth(2)
                .unwrap_or_else(|| panic!("{line:?} names no option"));
            key.rsplit('.')
                .next()
                .expect("a key ends in the relic's id")
                .to_string()
        })
        .collect()
}

#[test]
fn the_run_opens_on_the_blessing_the_game_rolled() {
    let want = offered();
    assert_eq!(want.len(), 3, "Neow offers two blessings and a curse");

    let seed = Seed::new(SEED).expect("the recording's seed is a seed");
    let mut rng: MegaRng = Blessing::<V>::stream(stream_seed(&seed), 0);
    let rolled = Blessing::<V>::roll(&mut rng, 1);

    let got: Vec<String> = rolled
        .options
        .iter()
        .map(|relic| {
            serde_json::to_value(relic)
                .expect("a relic serializes")
                .as_str()
                .expect("a relic id is a string")
                .to_string()
        })
        .collect();

    assert_eq!(got, want, "Neow offered something else");
}
