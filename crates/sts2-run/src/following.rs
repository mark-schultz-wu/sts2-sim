//! `sts2-run follow`: one recorded run, replayed.
//!
//! ```text
//! sts2-run follow <file>                   how many runs the file holds
//! sts2-run follow <file> <index>           how far the simulator follows it
//! sts2-run follow <file> <index> --at <step>
//! sts2-run follow <file> <index> --scenario
//! sts2-run follow <file> <index> --diff <dir>
//! ```
//!
//! Following a run prints `run <characters> <positions>`, then the verdict.
//! `agreed` follows it whole. `parted` names the position the two first
//! differ at as its step, and the move that led there. `panicked` names the
//! positions that agreed before it.
//!
//! `--at` prints the simulator's position before that move, and the move.
//! `--scenario` prints the run as a scenario's commands, up to the move that
//! led to its first split. `--diff` plays the run on the game in
//! `STS2_GAME_DIR` beside the simulator, picking each move from the game's
//! own list. Where the two first differ, it writes both under `<dir>`, as a
//! hunt does.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use sts2_core::{Decision, End, Gift, RunTable, SeatIndex, Setup, Step, Verdict};
use sts2_headless::{Config, LobbyTable, OracleTable, game_dir_from_env, recording};
use sts2_run::recorded::{Followed, Recorded, Run, follow};
use sts2_run::{Carried, Fault, Hunted, LogGuard, MovesLog, Recorded as Recording, catching};
use sts2_sim::SimTable;

/// Replay one recorded run.
#[derive(clap::Args)]
pub struct Args {
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
    /// Print the simulator's position before this move of the run.
    #[arg(long, requires = "index", value_name = "STEP")]
    at: Option<usize>,
}

/// Follows a run, or says why it does not.
pub fn run(args: Args) -> Result<(), String> {
    let Args {
        file,
        index,
        scenario,
        diff,
        at,
    } = args;
    let bytes = std::fs::read(&file).map_err(|e| format!("reading {}: {e}", file.display()))?;
    let recorded = Recorded::decode(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    let Some(index) = index else {
        println!("{}", recorded.runs.len());
        return Ok(());
    };
    let run = recorded
        .runs
        .get(index)
        .ok_or_else(|| format!("{} holds no run {index}", file.display()))?;
    let mode = match (at, scenario, diff) {
        (Some(step), _, _) => Mode::At(step),
        (_, true, _) => Mode::Scenario,
        (_, _, Some(dir)) => Mode::Diff(dir.join(index.to_string())),
        _ => Mode::Follow,
    };
    // The build whose game recorded the file, by its assembly hash. One arm
    // per build `sts2-content` compiles.
    match recorded.stamp.assembly {
        hash if hash == sts2_content::v0_107_1::VERSION.assembly_hash => {
            followed::<sts2_content::v0_107_1::V0_107_1>(run, mode)
        }
        hash if hash == sts2_content::v0_111_0::VERSION.assembly_hash => {
            followed::<sts2_content::v0_111_0::V0_111_0>(run, mode)
        }
        hash => Err(format!("{}: no build has assembly {hash}", file.display())),
    }
}

/// What to do with the run.
enum Mode {
    Follow,
    At(usize),
    Scenario,
    Diff(PathBuf),
}

/// `run` on build `V`, as `mode` says.
fn followed<V>(run: &Run, mode: Mode) -> Result<(), String>
where
    V: sts2_content::Build + sts2_content_wire::WireContentVersion,
    sts2_core::Seated<V>: Send,
{
    let start = run.start::<V>();
    match mode {
        Mode::Follow => {
            println!("run {} {}", run.characters(), run.digests.len());
            let agreed = AtomicUsize::new(0);
            let followed = catching(|| {
                let mut table = Carried::<SimTable<V>>::begin((), &start)?;
                follow(run, &mut table, true, |count| {
                    agreed.store(count, Ordering::Relaxed)
                })
            });
            let at = agreed.load(Ordering::Relaxed);
            println!("{}", verdict(followed, run.digests.len(), at));
        }
        Mode::At(step) => {
            let mut table = Carried::<SimTable<V>>::begin((), &start).map_err(|e| e.to_string())?;
            let played = replayed(run, &mut table, step, |_| {})?;
            let seat = run.seats.get(played).copied().unwrap_or(SeatIndex(0));
            println!("{:#?}", table.snapshot(seat).map_err(|e| e.to_string())?);
            if let Some(&chosen) = run.moves.get(step) {
                let mut moves = Vec::new();
                table.moves(seat, &mut moves).map_err(|e| e.to_string())?;
                println!("move {step}: {:?}", moves[usize::from(chosen)]);
            }
        }
        Mode::Scenario => {
            if run.players.len() > 1 {
                return Err("a scenario holds a run of one player".into());
            }
            let mut table = Carried::<SimTable<V>>::begin((), &start).map_err(|e| e.to_string())?;
            let moves = match follow(run, &mut table, true, |_| {}).map_err(|e| e.to_string())? {
                Followed::Whole => run.moves.len(),
                Followed::PartedAt { step, .. } => step,
            };
            let mut out = std::io::stdout().lock();
            let wrote = |e: std::io::Error| format!("writing the scenario: {e}");
            recording::began(&mut out, &start).map_err(wrote)?;
            for (relic, count) in sts2_run::carried::carried::<V>() {
                for _ in 0..count {
                    let grant = Step::<V>::Setup(Setup::Grant(Gift::Relic(relic)));
                    recording::append(&mut out, &grant).map_err(wrote)?;
                }
            }
            let mut table = Carried::<SimTable<V>>::begin((), &start).map_err(|e| e.to_string())?;
            let mut steps = Vec::new();
            replayed(run, &mut table, moves, |decision| steps.push(*decision))?;
            for decision in steps {
                recording::append(&mut out, &Step::Decision(decision)).map_err(wrote)?;
            }
        }
        Mode::Diff(dir) => {
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let game = game_dir_from_env().ok_or("STS2_GAME_DIR is unset")?;
            let verdict = match run.players.len() {
                1 => {
                    let mut config = Config::new(&game);
                    config.data_home = Some(dir.join("home"));
                    type Game<V> =
                        Carried<MovesLog<Hunted<V, Recording<V, LogGuard<OracleTable<V>>>>>>;
                    beside::<V, Game<V>>((false, (Some(dir), (None, config))), run, &start)
                }
                players => {
                    let seats = crate::lobby(&game, &dir, players).map_err(|e| e.to_string())?;
                    type Lobby<V> =
                        Carried<MovesLog<Hunted<V, Recording<V, LogGuard<LobbyTable<V>>>>>>;
                    beside::<V, Lobby<V>>((false, (Some(dir), (None, seats))), run, &start)
                }
            };
            println!("{verdict}");
        }
    }
    Ok(())
}

/// The first `upto` moves of `run` at `table`, unchecked, each handed to
/// `each` before it is played. How many were played.
fn replayed<V, T>(
    run: &Run,
    table: &mut T,
    upto: usize,
    mut each: impl FnMut(&Decision<V>),
) -> Result<usize, String>
where
    V: sts2_core::ContentVersion,
    T: RunTable<V>,
    Fault: From<T::Error>,
{
    let mut moves = Vec::new();
    let upto = upto.min(run.moves.len());
    for (step, (&at, &seat)) in run.moves[..upto].iter().zip(&run.seats).enumerate() {
        table
            .moves(seat, &mut moves)
            .map_err(|e| Fault::from(e).detail)?;
        let decision = moves[usize::from(at)];
        each(&decision);
        match table
            .play(seat, &decision)
            .map_err(|e| Fault::from(e).detail)?
        {
            Ok(_) => {}
            Err(refusal) => return Err(format!("{decision:?} refused at step {step}: {refusal}")),
        }
    }
    Ok(upto)
}

/// `run` played at a table of `T` that makes its own comparison, to the
/// first place it parts, as a verdict.
fn beside<V, T>(host: T::Host, run: &Run, start: &sts2_core::Start<V>) -> Verdict
where
    V: sts2_core::ContentVersion,
    T: RunTable<V, Error = Fault>,
{
    let played = catching(|| {
        let mut table = T::begin(host, start)?;
        let followed = follow(run, &mut table, false, |_| {});
        let ended = match &followed {
            Ok(_) => end(End::Agreed, run.moves.len(), String::new()),
            Err(fault) => end(fault.end, run.moves.len(), fault.detail.clone()),
        };
        Ok::<_, Fault>(table.finish(ended))
    });
    match played {
        Ok(Ok(verdict)) => verdict,
        Ok(Err(fault)) => end(fault.end, 0, fault.detail),
        Err((place, message)) => Verdict {
            place,
            ..end(End::Panicked, 0, message)
        },
    }
}

/// How following went, as a verdict whose step is the positions that agreed.
fn verdict(
    followed: Result<Result<Followed, Fault>, (Option<String>, String)>,
    positions: usize,
    agreed: usize,
) -> Verdict {
    match followed {
        Ok(Ok(Followed::Whole)) => end(End::Agreed, positions, String::new()),
        Ok(Ok(Followed::PartedAt { step, after })) => end(End::Parted, step, after),
        Ok(Err(fault)) => end(fault.end, agreed, fault.detail),
        Err((place, message)) => Verdict {
            place,
            ..end(End::Panicked, agreed, message)
        },
    }
}

/// A verdict on a replay, which says nothing of acts and floors.
fn end(end: End, step: usize, detail: String) -> Verdict {
    Verdict {
        end,
        step,
        act: 0,
        floor: 0,
        seat: None,
        place: None,
        detail,
    }
}
