//! The act map, against the one the game generated for the same seed.
//!
//! The first thing a run needs and the first thing worth checking. It comes
//! from one stream and no content. Getting it right is a statement about the
//! random numbers and the port, and nothing else.
//!
//! `tests/golden/<run>/map` is the act one map the oracle reported at the first
//! step of that recording. `cargo xtask golden` writes it beside the digests.
//!
//! `tests/maps.txt` is ten more seeds at every rung of the ascension ladder,
//! written by `cargo xtask maps`. One seed agreeing proves less than it looks.
//!
//! Every line of it is asserted, by two tests. The maps at the two elite counts
//! are generated and compared. The rest of the ladder is compared against the
//! first rung, which is the claim that lets the first test skip it.

use std::path::Path;

use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::ascension::AscensionLevel;
use sts2_core::Rules as _;
use sts2_core::rng::stream_seed;
use sts2_core::{ActMap as _, Seed};
use sts2_sim::map::ActMap;
use sts2_sim::rng::MegaRng;

type V = V0_107_1;

/// The recording the map is checked against, and the seed it was played on.
const RUN: &str = "0RACLE1-REGENT-2";
const SEED: &str = "0RACLE1";

/// One node per line: where it is, what it is, and what it leads to.
///
/// The same order the observation reports, so a failure reads as a diff down
/// the map rather than as a pile of unordered nodes.
fn render(map: &ActMap<V>) -> String {
    let mut out = String::new();
    for node in map.nodes() {
        let (at, kind, to) = (node.at, node.kind, &node.to);
        let edges = if to.is_empty() {
            "-".to_string()
        } else {
            to.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        };
        out.push_str(&format!("{at} {kind:?} {edges}\n"));
    }
    out
}

/// The act one map this seed generates at this ascension level, as lines.
fn generate(seed: &str, ascension: AscensionLevel) -> String {
    let seed = Seed::new(seed).expect("a fixture's seed is a seed");
    let mut rng = sts2_core::rng::named::<V, MegaRng>(stream_seed::<V>(&seed), "act_1_map");
    let layout = V::act_layout(sts2_content::v0_107_1::acts::Act::Overgrowth);
    render(&ActMap::<V>::generate(&mut rng, layout, ascension).unwrap())
}

/// The rung a fixture block names, by the game's own spelling.
fn level(name: &str) -> AscensionLevel {
    *AscensionLevel::ALL
        .iter()
        .find(|rung| format!("{rung:?}") == name)
        .unwrap_or_else(|| panic!("{name:?} is not a rung of the ladder"))
}

/// Where the first line differs, or nothing when the two agree.
///
/// Line by line, because the first line that differs is the one to look at and
/// a 68 line diff buries it.
fn differences(got: &str, want: &str) -> String {
    let mut out = String::new();
    for (at, (mine, theirs)) in got.lines().zip(want.lines()).enumerate() {
        if mine != theirs {
            out.push_str(&format!("  line {at}: got {mine:?}, want {theirs:?}\n"));
        }
    }
    if got.lines().count() != want.lines().count() {
        out.push_str(&format!(
            "  {} nodes, want {}\n",
            got.lines().count(),
            want.lines().count()
        ));
    }
    out
}

#[test]
fn the_first_act_matches_the_map_the_game_made() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(RUN)
        .join("map");
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));

    let got = generate(SEED, AscensionLevel::None);
    let lines = differences(&got, &want);
    assert!(
        lines.is_empty(),
        "{SEED} differs from {}\n{lines}",
        path.display()
    );
}

/// The fixture, as `(seed, rung) -> map`, in the order it was written.
fn fixture() -> (std::path::PathBuf, Vec<(String, String, String)>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/maps.txt");
    let file = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));

    let cases = file
        .split("\nseed ")
        .skip(1)
        .map(|block| {
            let (head, map) = block.split_once('\n').expect("a seed heads its own map");
            let (seed, rung) = head
                .split_once(' ')
                .unwrap_or_else(|| panic!("{head:?} names no ascension level"));
            (seed.to_string(), rung.to_string(), map.to_string())
        })
        .collect();
    (path, cases)
}

/// Ten seeds no recording covers, at the two elite counts a map is built with.
///
/// Every seed is checked before anything fails, and the report names each case
/// that differs. A generator wrong in one branch is right for some seeds and
/// wrong for others, and which ones is the first thing worth knowing.
///
/// The rest of the ladder belongs to the test below.
#[test]
fn every_seed_matches_at_both_elite_counts() {
    let (path, cases) = fixture();
    let checked = [AscensionLevel::None, AscensionLevel::SwarmingElites];

    let mut report = String::new();
    let mut done = 0;
    for (seed, rung, want) in &cases {
        let rung = level(rung);
        if !checked.contains(&rung) {
            continue;
        }
        done += 1;
        let lines = differences(&generate(seed, rung), want);
        if !lines.is_empty() {
            report.push_str(&format!("{seed} at {rung:?}:\n{lines}"));
        }
    }

    assert_eq!(
        done,
        20,
        "{} holds {done} of the two, not 20",
        path.display()
    );
    assert!(
        report.is_empty(),
        "maps differ from {}\n{report}",
        path.display()
    );
}

/// Nothing above the first rung of the ladder reaches a map.
///
/// The generator reads the ascension level once, for the elite count. So this
/// is the claim the test above rests on, checked against the game rather than
/// assumed.
///
/// A build that made a higher rung redraw a map fails here, naming it.
#[test]
fn the_ladder_reaches_no_map_above_the_first_rung() {
    let (path, cases) = fixture();

    let mut report = String::new();
    let mut rungs = 0;
    for (seed, rung, map) in &cases {
        if level(rung) <= AscensionLevel::SwarmingElites {
            continue;
        }
        rungs += 1;
        let (_, _, first) = cases
            .iter()
            .find(|(s, r, _)| s == seed && level(r) == AscensionLevel::SwarmingElites)
            .unwrap_or_else(|| panic!("{seed} has no map at the first rung"));
        if map != first {
            report.push_str(&format!("  {seed} at {rung} differs from SwarmingElites\n"));
        }
    }

    let want = 10 * (AscensionLevel::ALL.len() - 2);
    assert_eq!(
        rungs,
        want,
        "{} holds {rungs} maps above the first rung, not {want}",
        path.display()
    );
    assert!(
        report.is_empty(),
        "the ladder now reaches a map above its first rung, and the generator \
         does not know\n{report}"
    );
}
