//! The files committed beside a recording, against what the game said.
//!
//! `digests`, `map` and `room` are each derived from the transcript, and the
//! transcript sits under `target/` where no diff will ever show it. These are
//! the reviewable half. A re-pin that moves a digest, a map node or a Neow
//! option shows up in review, which is the point of writing them down.
//!
//! They are also what catches a stale transcript. The two came from different
//! games if they disagree, and the message says which way to resolve it.
//!
//! Set `STS2_UPDATE` to rewrite them from the transcript instead of asserting.
//! That is `just golden`, and it is what a re-pin runs.

use std::fmt::Write as _;
use std::path::Path;
use sts2_core::Digested;

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::Construct;
use sts2_core::Play;
use sts2_golden::{every_run, golden_dir, oracle_or_explain, recording, render_map, render_room};

type V = V0_107_1;

/// Whether to rewrite the files rather than hold them to account.
fn updating() -> bool {
    std::env::var_os("STS2_UPDATE").is_some()
}

#[test]
fn the_committed_files_say_what_the_game_said() {
    let mut rewritten = Vec::new();

    for name in every_run().expect("the recorded runs are readable") {
        let recorded = recording::<V>(&name).expect("a recorded run reads");
        let mut oracle = oracle_or_explain(&name, recorded.start());

        let mut digests = String::new();
        let opening = Play::<V>::observe(&mut oracle)
            .unwrap_or_else(|e| panic!("{name}: observing the start: {e}"));
        let map = render_map(&opening.observation.map);
        let room = render_room(opening.observation.room.as_ref());
        for (at, step) in recorded.steps().iter().enumerate() {
            oracle
                .step(step)
                .unwrap_or_else(|e| panic!("{name}: step {at}: {e}"))
                .unwrap_or_else(|refusal| panic!("{name}: step {at}: {refusal}"));
            let standing = Play::<V>::observe(&mut oracle)
                .unwrap_or_else(|e| panic!("{name}: step {at}: {e}"));

            let _ = writeln!(digests, "{}", standing.observation.rules_digest());
        }

        for (file, produced) in [("digests", digests), ("map", map), ("room", room)] {
            let path = golden_dir().join(&name).join(file);
            if updating() {
                if std::fs::read_to_string(&path).ok().as_deref() != Some(produced.as_str()) {
                    std::fs::write(&path, &produced)
                        .unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
                    rewritten.push(format!("{name}/{file}"));
                }
                continue;
            }
            held(&path, &produced, &name);
        }
    }

    assert!(
        !updating(),
        "rewrote {} file(s) from the transcript: {}",
        rewritten.len(),
        if rewritten.is_empty() {
            "none, they already agreed".to_string()
        } else {
            rewritten.join(", ")
        }
    );
}

/// One committed file against what the transcript produces for it.
fn held(path: &Path, produced: &str, name: &str) {
    let committed =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    if committed == produced {
        return;
    }

    let at = committed
        .lines()
        .zip(produced.lines())
        .position(|(a, b)| a != b);
    let where_ = match at {
        Some(line) => format!(
            "line {}:\n  committed {}\n  the game  {}",
            line + 1,
            committed.lines().nth(line).unwrap_or(""),
            produced.lines().nth(line).unwrap_or(""),
        ),
        None => format!(
            "{} committed lines against {} from the game",
            committed.lines().count(),
            produced.lines().count()
        ),
    };

    panic!(
        "{name}: {} disagrees with what the game said, at {where_}\n\n\
         Either the build moved and this is the re-pin, in which case \
         `just golden` rewrites it; or the transcript under target/ is left \
         over from another build, in which case `cargo clean` drops it.",
        path.display(),
    );
}
