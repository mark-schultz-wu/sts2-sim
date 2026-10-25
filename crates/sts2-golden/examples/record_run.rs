//! One run the game plays at random, for `cargo xtask record` and `hunt`.
//!
//! ```text
//! record_run <seed> <character> <out> [--data-home <dir>] [--ascension <id>]
//!     [--modifiers <id>,...] [--hunt]
//! ```
//!
//! The game plays the run, carrying what the census's runs carry. It picks
//! each move from its own legal list, the way the census picks from the
//! simulator's. The run goes to `<out>` as a file of one recorded run. See
//! `sts2_golden::recorded`.
//!
//! The build is `STS2_GAME_VERSION`'s, the setting that chose
//! `STS2_GAME_DIR`, and v0.107.1 when it is unset.
//!
//! It exits 0 with the run written, and 3 when the game closed the connection
//! or stopped answering, which is worth playing again. It exits 1 for anything
//! else, which is not. A command
//! the game logged an error on stops the run as well, because what it left
//! the run holding cannot be trusted. `--hunt` is [`Outcome`]'s.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;

use clap::Parser;
use sts2_content_wire::WireContentVersion;
use sts2_core::{Construct as _, Gift, Observed as _, Play as _, SeatIndex, Seed, Setup, Start};
use sts2_golden::recorded::{Ending, Recorded, Run, named, parted, position};
use sts2_golden::scenario::Stamp;
use sts2_headless::{Config, Oracle, game_dir_from_env};

#[path = "carried/mod.rs"]
mod carried;
use carried::carried;

/// Decisions a run takes before this gives up on it, as the census's runner.
const STEPS: usize = 20_000;

/// Play one run on the game at random, and keep it.
#[derive(Parser)]
struct Cli {
    /// The seed, such as `C000000042`.
    #[arg(value_parser = seed)]
    seed: Seed,
    /// The character, by id, such as `IRONCLAD`.
    character: String,
    /// Where to write the run.
    out: PathBuf,
    /// A directory for the game's own profile, so several run at once.
    #[arg(long, value_name = "DIR")]
    data_home: Option<PathBuf>,
    /// The ascension level, by the id the game spells it with.
    #[arg(long, default_value = "None")]
    ascension: String,
    /// The modifiers the run starts with, by the ids the game spells them
    /// with. Character Cards names its character after a colon, as
    /// `CHARACTER_CARDS:DEFECT`.
    #[arg(long, value_delimiter = ',')]
    modifiers: Vec<String>,
    /// Play the simulator beside the game, and keep the run only where the
    /// two part.
    #[arg(long)]
    hunt: bool,
}

/// The run so far and where it goes, which the panic hook writes when the
/// simulator panics.
static PLAYED: Mutex<Option<(PathBuf, Recorded)>> = Mutex::new(None);

/// What [`PLAYED`]'s run does, under its lock.
fn played<R>(with: impl FnOnce(&mut Run) -> R) -> R {
    let mut played = PLAYED.lock().expect("no panic holds the run");
    let (_, recorded) = played.as_mut().expect("a run is being played");
    with(&mut recorded.runs[0])
}

/// How the run went. `--hunt` compares the simulator with the game before
/// each move, and prints a line that starts with the outcome's name.
enum Outcome {
    /// Written whole.
    Kept,
    /// Not written: the simulator agreed with the game to the end. It exits
    /// 0.
    Agreed,
    /// Written up to the game's position where the simulator first parted
    /// from it. It exits 2. A simulator that panics has the run written as
    /// far as the game got, and prints `panicked`, and it exits 101.
    Parted,
}

fn seed(text: &str) -> Result<Seed, sts2_core::seed::Error> {
    Seed::new(text)
}

/// Content by the id the game spells it with.
fn parse<V: WireContentVersion, T: serde::de::DeserializeOwned>(id: &str) -> Result<T, Error<V>> {
    serde_json::from_value(serde_json::Value::String(id.to_owned())).map_err(|source| Error::Id {
        id: id.to_owned(),
        source,
    })
}

#[derive(Debug, thiserror::Error)]
enum Error<V: WireContentVersion> {
    #[error("{id:?} names nothing: {source}")]
    Id {
        id: String,
        source: serde_json::Error,
    },
    #[error("STS2_GAME_DIR is unset")]
    NoGame,
    #[error("the game: {0}")]
    Game(#[from] sts2_headless::Error),
    #[error("the game refused {what}: {refusal}")]
    Refused {
        what: String,
        refusal: sts2_core::Refusal<V>,
    },
    #[error("step {step}: the game logged {errors:?}")]
    Logged { step: usize, errors: Vec<String> },
    #[error("step {step}: a run with nothing legal to do")]
    NothingLegal { step: usize },
    #[error("writing the run: {0}")]
    Write(#[from] std::io::Error),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // One arm per build `sts2-content` compiles.
    let version = std::env::var("STS2_GAME_VERSION").unwrap_or_else(|_| "v0.107.1".to_string());
    match version.as_str() {
        "v0.107.1" => exit(record::<sts2_content::v0_107_1::V0_107_1>(cli)),
        "v0.111.0" => exit(record::<sts2_content::v0_111_0::V0_111_0>(cli)),
        other => {
            eprintln!("error: no build {other:?}; try v0.107.1 or v0.111.0");
            ExitCode::FAILURE
        }
    }
}

/// The exit status for how the run went, with the error on stderr.
fn exit<V: WireContentVersion>(recorded: Result<Outcome, Error<V>>) -> ExitCode {
    match recorded {
        Ok(Outcome::Kept | Outcome::Agreed) => ExitCode::SUCCESS,
        Ok(Outcome::Parted) => ExitCode::from(2),
        Err(e) => {
            eprintln!("error: {e}");
            // Only a game that went away is played again. One that answered
            // with something this cannot read said something wrong.
            match e {
                Error::Game(sts2_headless::Error::GameGone | sts2_headless::Error::Timeout(_)) => {
                    ExitCode::from(3)
                }
                _ => ExitCode::FAILURE,
            }
        }
    }
}

fn record<V: WireContentVersion + sts2_content::Build>(cli: Cli) -> Result<Outcome, Error<V>> {
    let Cli {
        seed,
        character,
        out,
        data_home,
        ascension,
        modifiers,
        hunt,
    } = cli;
    let start = Start::<V>::solo(
        seed.clone(),
        parse(&character)?,
        parse(&ascension)?,
        modifiers
            .iter()
            .map(|text| {
                sts2_golden::recorded::read_modifier(text).map_err(|source| Error::Id {
                    id: text.clone(),
                    source,
                })
            })
            .collect::<Result<_, _>>()?,
        sts2_golden::random::wins(seed.as_str()),
    );
    let mut config = Config::new(game_dir_from_env().ok_or(Error::NoGame)?);
    config.data_home = data_home;
    let mut pick = sts2_golden::random::Pick::for_seed(start.seed.as_str());
    let mut game = Oracle::<V>::begin(config, &start)?;
    for (relic, count) in carried::<V>() {
        for _ in 0..count {
            game.setup(&Setup::Grant(Gift::Relic(relic)))?
                .map_err(|refusal| Error::Refused {
                    what: format!("{relic:?}"),
                    refusal,
                })?;
        }
    }

    // Each move's reply carries where it left the run, so only the start is
    // asked for.
    let mut seen = game.observe()?;
    let greeting = game.greeting();
    *PLAYED.lock().expect("no panic holds the run") = Some((
        out.clone(),
        Recorded {
            stamp: Stamp {
                assembly: greeting.assembly_hash,
                content: i64::from(greeting.content_hash),
            },
            runs: vec![Run {
                digests: vec![position(&seen)],
                ..Run::of(&start, Ending::Parted)
            }],
        },
    ));
    let mut sim = hunt.then(|| simulator::<V>(&start));

    let mut after = String::from("the start");
    let ending = loop {
        let step = played(|run| run.moves.len());
        if let Some(sim) = sim.as_mut() {
            let simulated = sim
                .observe()
                .unwrap_or_else(|e| panic!("observing step {step}: {e}"));
            if position(&simulated) != position(&seen) {
                println!(
                    "parted {step} after {after} in {}",
                    parted(&simulated, &seen)
                );
                break Ending::Parted;
            }
        }
        if seen.observation.run_over() {
            break match seen.observation.victory() {
                Some(true) => Ending::Won,
                _ => Ending::Lost,
            };
        }
        if step == STEPS {
            break Ending::Capped;
        }
        let legal = &seen.observation.moves;
        if legal.is_empty() {
            return Err(Error::NothingLegal { step });
        }
        let room = sts2_golden::random::making_room(
            seen.asking.as_ref(),
            seen.observation.potions.len(),
            seen.observation.potion_slots,
        );
        let at = pick.choose(legal, room);
        let decision = legal[at];
        after = named(&seen, &decision);
        seen = game
            .play(&decision)?
            .map_err(|refusal| Error::Refused {
                what: format!("{decision:?} at step {step}"),
                refusal,
            })?
            .snapshot();
        // The move and where the game went with it, together, so a panic
        // below leaves a run that reaches the move it panicked on.
        played(|run| {
            run.moves
                .push(u16::try_from(at).expect("a list of moves fits a u16"));
            run.seats.push(SeatIndex(0));
            run.digests.push(position(&seen));
        });
        if !game.logged().is_empty() {
            // A hunt keeps the run up to the move the game logged on, so the
            // move can be found again.
            if hunt {
                let (out, recorded) = PLAYED
                    .lock()
                    .expect("no panic holds the run")
                    .take()
                    .expect("a run is being played");
                std::fs::write(&out, recorded.encode())?;
            }
            return Err(Error::Logged {
                step,
                errors: game.logged().to_vec(),
            });
        }
        if let Some(sim) = sim.as_mut() {
            sim.play(&decision)
                .unwrap_or_else(|e| panic!("playing {decision:?} at step {step}: {e}"))
                .unwrap_or_else(|refusal| panic!("{decision:?} refused at step {step}: {refusal}"));
        }
    };

    let (out, mut recorded) = PLAYED
        .lock()
        .expect("no panic holds the run")
        .take()
        .expect("a run was played");
    let moves = recorded.runs[0].moves.len();
    recorded.runs[0].ending = ending;
    if hunt && ending != Ending::Parted {
        println!("agreed {ending:?} after {moves} moves");
        return Ok(Outcome::Agreed);
    }
    std::fs::write(&out, recorded.encode())?;
    Ok(match ending {
        Ending::Parted => Outcome::Parted,
        _ => {
            println!("{ending:?} after {moves} moves");
            Outcome::Kept
        }
    })
}

/// The simulator on `start`, carrying what the game's run carries. A panic
/// from here on writes the run as far as the game got.
fn simulator<V: sts2_content::Build>(start: &Start<V>) -> sts2_sim::Sim<V> {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Ok(mut played) = PLAYED.try_lock()
            && let Some((out, recorded)) = played.as_mut()
        {
            let wrote = std::fs::write(out, recorded.encode());
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|text| text.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            let step = recorded.runs[0].moves.len();
            println!("panicked {step} {}", message.lines().next().unwrap_or(""));
            if let Err(e) = wrote {
                eprintln!("writing the run: {e}");
            }
        }
        report(info);
    }));
    let mut sim = sts2_sim::Sim::<V>::begin((), start).expect("the simulator starts the run");
    for (relic, count) in carried::<V>() {
        for _ in 0..count {
            sim.setup(&Setup::Grant(Gift::Relic(relic)))
                .expect("the simulator grants")
                .expect("and does not refuse");
        }
    }
    sim
}
