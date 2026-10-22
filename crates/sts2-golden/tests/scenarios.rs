//! The simulator, against the game, on a position nobody played to.
//!
//! A run takes an hour to reach a fight with two debuffs on the player. A
//! scenario reaches it in four lines, because the game has debug commands for
//! exactly that and the mod exposes them. So the state a scenario builds is a
//! state the game builds, which is what makes holding the simulator to it
//! worth anything.
//!
//! What the game answered lives in `scenarios.bin`, one file for the suite.
//! This reads it and needs no game. `just scenarios` is what writes it, and
//! that is the only thing here that launches one.
//!
//! Set `STS2_UPDATE` to write instead of assert.
//!
//! `tests/scenarios/README.md` says how to write one and what makes a scenario
//! worth its bytes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use sts2_core::Digested;

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, ContentVersion, Play, Section, Snapshot};
use sts2_golden::scenario::{Expected, Seen, Stamp, Step};
use sts2_golden::suite::read_suite;

type V = V0_107_1;

fn updating() -> bool {
    std::env::var_os("STS2_UPDATE").is_some()
}

fn suite_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios")
}

fn expected_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios.bin")
}

/// What every step a run of this recording passes through leaves showing.
fn walk<C, E>(name: &str, driven: &mut C, steps: &[sts2_core::Step<V>]) -> Vec<Seen>
where
    C: Construct<V, Error = E>,
    E: std::fmt::Display,
{
    steps
        .iter()
        .map(|step| {
            driven
                .step(step)
                .unwrap_or_else(|e| panic!("{name}: {e}"))
                .unwrap_or_else(|refusal| panic!("{name}: refused: {refusal}"));
            let seen: Snapshot<V> =
                Play::<V>::observe(driven).unwrap_or_else(|e| panic!("{name}: observing: {e}"));
            Seen {
                sections: seen.observation.sections().to_vec(),
                question: sts2_core::Digest::of_question_and_pile(seen.asking.as_ref()),
            }
        })
        .collect()
}

#[test]
fn the_simulator_reaches_the_positions_the_game_reaches() {
    let written = read_suite(&suite_dir()).expect("the suite is readable");
    // A scenario file carries no build stamp. See `suite.rs`.
    let stamp =
        serde_json::to_string(&sts2_headless::Stamp::of(&V::VERSION)).expect("a stamp is json");

    // An update writes every answer afresh, so what the file held, in
    // whatever layout, is never read.
    let held = match std::fs::read(expected_path()) {
        _ if updating() => Expected::default(),
        Ok(bytes) => Expected::decode(&bytes).expect("the answers parse"),
        Err(e) => panic!(
            "no recorded answers: {e}\n    \
             `just scenarios` records them, which needs the game and the mod."
        ),
    };

    let mut fresh = Expected {
        stamp: Stamp {
            assembly: V::VERSION.assembly_hash,
            content: i64::from(V::VERSION.content_hash),
        },
        scenarios: BTreeMap::new(),
    };
    let mut wrong = Vec::new();

    for entry in &written {
        let name = &entry.name;
        let recording = format!("{stamp}\n{}", entry.commands);
        let recorded =
            sts2_headless::recording::read::<V>(std::io::Cursor::new(recording.as_bytes()))
                .unwrap_or_else(|e| panic!("{name}: {e}"));

        // An update asks the game alone, so a simulator that cannot play a
        // scenario yet leaves its recording to be written all the same.
        if updating() {
            let mut oracle = sts2_golden::oracle_or_explain::<V>(name, recorded.start());
            let answered = walk(name, &mut oracle, recorded.steps());
            let mut steps = Vec::new();
            let mut before: Option<Seen> = None;
            for seen in answered {
                steps.push(Step::between(before.as_ref(), &seen));
                before = Some(seen);
            }
            fresh.scenarios.insert(name.clone(), steps);
            continue;
        }

        let steps = held
            .scenarios
            .get(name)
            .unwrap_or_else(|| panic!("{name} has no recorded answers; run `just scenarios`"));
        let mut sim = sts2_sim::Sim::<V>::begin((), recorded.start())
            .unwrap_or_else(|e| panic!("{name}: starting: {e}"));
        let walked = walk(name, &mut sim, recorded.steps());
        if let Some(why) = compare(steps, &walked) {
            wrong.push(format!("{name}: {why}"));
        }
    }

    if updating() {
        std::fs::write(expected_path(), fresh.encode()).expect("the answers are writable");
        println!("recorded {} scenarios", fresh.scenarios.len());
        return;
    }

    assert!(
        held.stamp
            == Stamp {
                assembly: V::VERSION.assembly_hash,
                content: i64::from(V::VERSION.content_hash),
            },
        "the answers came from another build; run `just scenarios`"
    );
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Where a scenario parted, named by section, or as `question`.
///
/// Only the parts are named. What the game said is a hash and nothing more, so
/// a failure says which part moved and leaves the simulator to print its own.
fn compare(want: &[Step], got: &[Seen]) -> Option<String> {
    if want.len() != got.len() {
        return Some(format!(
            "{} steps recorded, {} played",
            want.len(),
            got.len()
        ));
    }

    let mut standing: BTreeMap<Section, sts2_core::Digest> = BTreeMap::new();
    let mut asked = None;
    for (at, (step, seen)) in want.iter().zip(got).enumerate() {
        standing.extend(step.moved.iter().copied());
        asked = step.question.or(asked);
        let mut named: Vec<&str> = seen
            .sections
            .iter()
            .filter(|(section, now)| standing.get(section) != Some(now))
            .map(|(section, _)| section.name())
            .collect();
        if asked != Some(seen.question) {
            named.push("question");
        }
        if !named.is_empty() {
            return Some(format!("step {at} differs in {}", named.join(", ")));
        }
    }
    None
}
