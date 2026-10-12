//! The committed file of runs the game played, read back.
//!
//! `cargo xtask record` writes it, on purpose and never in CI. This holds it
//! to what the recorder plays: every census seed for every character, each
//! run whole.

use std::collections::BTreeMap;

use sts2_golden::recorded::Recorded;

/// The seeds a character plays, as `cargo xtask record` plays them.
const RUNS: usize = 300;

#[test]
fn the_committed_file_holds_every_census_run() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/recorded.bin");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let recorded = Recorded::decode(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    let mut seeds: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for run in &recorded.runs {
        seeds.entry(&run.character).or_default().push(&run.seed);
    }
    let want: Vec<String> = (0..RUNS).map(|at| format!("C{at:09}")).collect();
    for character in ["DEFECT", "IRONCLAD", "NECROBINDER", "REGENT", "SILENT"] {
        let mut played = seeds.remove(character).unwrap_or_default();
        played.sort_unstable();
        assert_eq!(played, want, "{character}'s seeds");
    }
    assert!(seeds.is_empty(), "runs for no census character: {seeds:?}");
}
