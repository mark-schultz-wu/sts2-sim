//! One run the game plays at random, for `cargo xtask record`.
//!
//! ```text
//! record_run <seed> <character> <out> [--data-home <dir>]
//! ```
//!
//! The game plays the run, carrying what the census's runs carry. It picks
//! each move from its own legal list, the way the census picks from the
//! simulator's. The run goes to `<out>` as a file of one recorded run. See
//! `sts2_golden::recorded`.
//!
//! It exits 0 with the run written, and 3 when the game closed the connection
//! or stopped answering, which is worth playing again. It exits 1 for anything
//! else, which is not. A command
//! the game logged an error on stops the run as well, because what it left
//! the run holding cannot be trusted.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::characters::Character;
use sts2_core::{Construct as _, Gift, Observed as _, Play as _, Seed, Setup, Start};
use sts2_golden::recorded::{Ending, Recorded, Run, position};
use sts2_golden::scenario::Stamp;
use sts2_headless::{Config, Oracle, game_dir_from_env};

#[path = "carried/mod.rs"]
mod carried;
use carried::CARRIED;

type V = V0_107_1;

/// Decisions a run takes before this gives up on it, as the census's runner.
const STEPS: usize = 20_000;

/// Play one run on the game at random, and keep it.
#[derive(Parser)]
struct Cli {
    /// The seed, such as `C000000042`.
    #[arg(value_parser = seed)]
    seed: Seed,
    /// The character, by id, such as `IRONCLAD`.
    #[arg(value_parser = character)]
    character: Character,
    /// Where to write the run.
    out: PathBuf,
    /// A directory for the game's own profile, so several run at once.
    #[arg(long, value_name = "DIR")]
    data_home: Option<PathBuf>,
    /// The ascension level, by the id the game spells it with.
    #[arg(long, value_parser = ascension, default_value = "None")]
    ascension: sts2_content::v0_107_1::ascension::AscensionLevel,
}

/// An ascension level by the id the game spells it with.
fn ascension(
    id: &str,
) -> Result<sts2_content::v0_107_1::ascension::AscensionLevel, serde_json::Error> {
    serde_json::from_value(serde_json::Value::String(id.to_owned()))
}

fn seed(text: &str) -> Result<Seed, sts2_core::seed::Error> {
    Seed::new(text)
}

/// A character by the id the game spells it with.
fn character(id: &str) -> Result<Character, serde_json::Error> {
    serde_json::from_value(serde_json::Value::String(id.to_owned()))
}

#[derive(Debug, thiserror::Error)]
enum Error {
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
    match record(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
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

fn record(cli: Cli) -> Result<(), Error> {
    let Cli {
        seed,
        character,
        out,
        data_home,
        ascension,
    } = cli;
    let mut config = Config::new(game_dir_from_env().ok_or(Error::NoGame)?);
    config.data_home = data_home;
    let start = Start::<V> {
        wins: sts2_golden::random::wins(seed.as_str()),
        seed,
        character,
        ascension,
        modifiers: Vec::new(),
    };
    let mut pick = sts2_golden::random::Pick::for_seed(start.seed.as_str());
    let mut game = Oracle::<V>::begin(config, &start)?;
    for (relic, count) in CARRIED {
        for _ in 0..count {
            game.setup(&Setup::Grant(Gift::Relic(relic)))?
                .map_err(|refusal| Error::Refused {
                    what: format!("{relic:?}"),
                    refusal,
                })?;
        }
    }

    let mut digests = Vec::new();
    let mut moves = Vec::new();
    // Each move's reply carries where it left the run, so only the start is
    // asked for.
    let mut seen = game.observe()?;
    let ending = loop {
        let step = moves.len();
        digests.push(position(&seen));
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
        let at = pick.choose(legal);
        let decision = legal[at];
        moves.push(u16::try_from(at).expect("a list of moves fits a u16"));
        seen = game
            .play(&decision)?
            .map_err(|refusal| Error::Refused {
                what: format!("{decision:?} at step {step}"),
                refusal,
            })?
            .snapshot();
        if !game.logged().is_empty() {
            return Err(Error::Logged {
                step,
                errors: game.logged().to_vec(),
            });
        }
    };

    let greeting = game.greeting();
    let recorded = Recorded {
        stamp: Stamp {
            assembly: greeting.assembly_hash,
            content: i64::from(greeting.content_hash),
        },
        runs: vec![Run {
            character: id(&start.character),
            seed: start.seed.as_str().to_owned(),
            ascension: id(&start.ascension),
            wins: start.wins,
            ending,
            digests,
            moves,
        }],
    };
    std::fs::write(&out, recorded.encode())?;
    println!("{ending:?} after {} moves", recorded.runs[0].moves.len());
    Ok(())
}

/// An id as the game spells it, which is how serde writes it.
fn id(value: &impl serde::Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(id)) => id,
        other => panic!("an id that is not a string: {other:?}"),
    }
}
