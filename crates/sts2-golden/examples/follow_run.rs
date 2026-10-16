//! How far the simulator follows one recorded run, for `cargo xtask record`.
//!
//! ```text
//! follow_run <file> [index]
//! follow_run <file> <index> --scenario
//! follow_run <file> <index> --diff <dir>
//! ```
//!
//! It prints one of these, where `after` names the move that led to the
//! first position that differs:
//!
//! ```text
//! <character> <positions> whole
//! <character> <positions> parted <step> <after>
//! <character> <positions> panicked <agreed>
//! ```
//!
//! Without an index it prints how many runs the file holds. The caller reads
//! a panic's message off stderr, so this runs in a process of its own.
//!
//! `--diff` plays the run on the game in `STS2_GAME_DIR` and the simulator
//! side by side. The game picks each move from its own list. Where the two
//! first differ, it names the sections and writes `<dir>/<index>.sim` and
//! `.game`. A split with no panic needs this on a build no scenario replays.
//!
//! `--scenario` prints the run as a scenario's commands, up to the move that
//! led to its first split. Up to there the simulator's list is the game's,
//! so its moves are the game's. `cargo xtask split` writes it out.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use std::sync::atomic::{AtomicUsize, Ordering};
use sts2_core::{Construct as _, Digested as _, Gift, Play as _, Setup, Step};

use sts2_golden::recorded::{Followed, Recorded, Run, follow, follow_noting};
use sts2_headless::recording;
use sts2_sim::Sim;

#[path = "carried/mod.rs"]
mod carried;
use carried::carried;

/// Replay one recorded run on the simulator.
#[derive(Parser)]
struct Cli {
    /// The file of recorded runs.
    file: PathBuf,
    /// Which run in it, counted from zero.
    index: Option<usize>,
    /// Print the run as a scenario, up to its first split.
    #[arg(long, requires = "index")]
    scenario: bool,
    /// Play the run on the game beside the simulator, and write both sides
    /// where they first differ, to this directory.
    #[arg(long, requires = "index", value_name = "DIR")]
    diff: Option<PathBuf>,
}

fn main() -> ExitCode {
    let Cli {
        file,
        index,
        scenario,
        diff,
    } = Cli::parse();
    let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("reading {}: {e}", file.display()));
    let recorded = Recorded::decode(&bytes).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let Some(index) = index else {
        println!("{}", recorded.runs.len());
        return ExitCode::SUCCESS;
    };
    let run = &recorded.runs[index];
    // The build whose game recorded the file, by its assembly hash. One arm
    // per build `sts2-content` compiles.
    match recorded.stamp.assembly {
        hash if hash == sts2_content::v0_107_1::VERSION.assembly_hash => {
            followed::<sts2_content::v0_107_1::V0_107_1>(run, index, scenario, diff.as_deref());
        }
        hash if hash == sts2_content::v0_111_0::VERSION.assembly_hash => {
            followed::<sts2_content::v0_111_0::V0_111_0>(run, index, scenario, diff.as_deref());
        }
        hash => {
            eprintln!("{}: no build has assembly {hash}", file.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

/// `run` followed on build `V`, printed as a scenario, or played beside the
/// game.
fn followed<V: sts2_content::Build + sts2_content_wire::WireContentVersion>(
    run: &Run,
    index: usize,
    scenario: bool,
    diff: Option<&std::path::Path>,
) {
    if let Some(dir) = diff {
        beside_the_game::<V>(run, index, dir);
        return;
    }
    if scenario {
        print_scenario::<V>(run);
        return;
    }
    // First, so a panic below still says whose run it was, and how long.
    print!("{} {} ", run.character, run.digests.len());
    std::io::Write::flush(&mut std::io::stdout()).expect("stdout takes a line");
    static AGREED: AtomicUsize = AtomicUsize::new(0);
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        println!("panicked {}", AGREED.load(Ordering::Relaxed));
        report(info);
    }));
    let mut sim = begun::<V>(run);
    match follow_noting(run, &mut sim, |agreed| {
        AGREED.store(agreed, Ordering::Relaxed)
    }) {
        Followed::Whole => println!("whole"),
        Followed::PartedAt { step, after } => println!("parted {step} {after}"),
    }
}

/// The simulator on `run`'s start, carrying what the run carried.
fn begun<V: sts2_content::Build>(run: &Run) -> Sim<V> {
    let mut sim = Sim::<V>::begin((), &run.start()).expect("the run starts");
    for (relic, count) in carried::<V>() {
        for _ in 0..count {
            sim.setup(&Setup::Grant(Gift::Relic(relic)))
                .expect("a grant applies")
                .expect("and is not refused");
        }
    }
    sim
}

/// `run` as a scenario's commands: its start, what it carried, and its moves
/// up to and including the one that led to its first split.
fn print_scenario<V: sts2_content::Build + sts2_content_wire::WireContentVersion>(run: &Run) {
    let moves = match follow(run, &mut begun::<V>(run)) {
        Followed::Whole => run.moves.len(),
        Followed::PartedAt { step, .. } => step,
    };
    let mut out = std::io::stdout().lock();
    let wrote = "stdout takes the scenario";
    recording::began(&mut out, &run.start::<V>()).expect(wrote);
    for (relic, count) in carried::<V>() {
        for _ in 0..count {
            let grant = Step::<V>::Setup(Setup::Grant(Gift::Relic(relic)));
            recording::append(&mut out, &grant).expect(wrote);
        }
    }
    let mut sim = begun::<V>(run);
    for &at in &run.moves[..moves] {
        let seen = sim.observe().expect("the simulator observes");
        let decision = seen.observation.moves[usize::from(at)];
        recording::append(&mut out, &Step::Decision(decision)).expect(wrote);
        sim.play(&decision)
            .expect("the move plays")
            .expect("and is not refused");
    }
}

/// `run` played on the game and the simulator at once. At the first
/// position they differ at, this names the sections and writes both
/// snapshots under `dir`.
fn beside_the_game<V: sts2_content::Build + sts2_content_wire::WireContentVersion>(
    run: &Run,
    index: usize,
    dir: &std::path::Path,
) {
    let start = run.start::<V>();
    let home = sts2_headless::game_dir_from_env().expect("STS2_GAME_DIR names the game");
    let mut game = sts2_headless::Oracle::<V>::begin(sts2_headless::Config::new(home), &start)
        .expect("the game starts the run");
    for (relic, count) in carried::<V>() {
        for _ in 0..count {
            game.setup(&Setup::Grant(Gift::Relic(relic)))
                .expect("the game grants")
                .expect("and does not refuse");
        }
    }
    let mut sim = begun::<V>(run);
    for at in 0..=run.moves.len() {
        let played = game.observe().expect("the game observes");
        let simulated = sim.observe().expect("the simulator observes");
        let sections: Vec<String> = simulated
            .observation
            .sections()
            .iter()
            .zip(played.observation.sections().iter())
            .filter(|(a, b)| a != b)
            .map(|(a, _)| format!("{:?}", a.0))
            .collect();
        let asked = sts2_core::Digest::of_question(simulated.asking.as_ref())
            != sts2_core::Digest::of_question(played.asking.as_ref());
        if !sections.is_empty() || asked {
            std::fs::create_dir_all(dir).expect("the directory is made");
            for (end, snapshot) in [("sim", &simulated), ("game", &played)] {
                let path = dir.join(format!("{index}.{end}"));
                std::fs::write(&path, format!("{snapshot:#?}\n")).expect("the snapshot is written");
            }
            let question = if asked { " and the question" } else { "" };
            println!("step {at} differs in {}{question}", sections.join(", "));
            return;
        }
        let Some(&chosen) = run.moves.get(at) else {
            break;
        };
        let decision = played.observation.moves[usize::from(chosen)];
        game.play(&decision)
            .expect("the game plays")
            .expect("and does not refuse");
        sim.play(&decision)
            .expect("the simulator plays")
            .expect("and does not refuse");
    }
    println!("the two sides agree at every step");
}
