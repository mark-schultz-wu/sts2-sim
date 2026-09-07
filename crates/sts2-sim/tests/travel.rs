//! Travel legality before room entry begins.

use std::{fs::File, io::BufReader, path::Path};

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Coord, Decision, Play, Reason};
use sts2_sim::SimBuilder;

type V = V0_107_1;

#[test]
fn an_unreachable_destination_is_refused_without_moving_the_run() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/0RACLE1-REGENT-2/run.jsonl");
    let recording = sts2_headless::recording::read::<V>(BufReader::new(
        File::open(&path).unwrap_or_else(|e| panic!("opening {}: {e}", path.display())),
    ))
    .expect("the golden recording parses");
    // The first step starts the run, so it is played on the builder. The
    // simulator exists only once a step has started one.
    let steps = recording.steps();
    let mut builder = SimBuilder::<V>::new();
    builder
        .step(&steps[0])
        .expect("the simulator handles the opening step")
        .expect("the recording is legal");
    let mut sim = builder.build().expect("the opening step started a run");
    for step in steps.iter().take(3).skip(1) {
        sim.step(step)
            .expect("the simulator handles the setup")
            .expect("the recording is legal");
    }
    let before = sim
        .observe()
        .expect("the run can be observed")
        .expect("the run exists");
    let nowhere = Coord::new(0, 0);

    let refused = sim
        .play(&Decision::Travel { to: nowhere })
        .expect("travel was answered")
        .expect_err("the node is not reachable");

    assert_eq!(refused.what, "travel");
    assert_eq!(refused.reason, Reason::NotTravelable { at: nowhere });
    assert_eq!(
        sim.observe().expect("the run can still be observed"),
        Ok(before)
    );
}
