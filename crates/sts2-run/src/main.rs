//! `sts2-run`: one run, played at a table by a player, to a verdict.
//!
//! ```text
//! sts2-run <seed> <characters> [--build v0.107.1] [--table sim]
//!          [--player random] [--carried] [--steps N]
//!          [--ascension ID] [--modifiers ID,...]
//! ```
//!
//! `<characters>` is one character for a solo run, or several joined by
//! commas for a run of several players, one seat each. The last line on
//! stdout is the run's [`Verdict`]. An unported path panics, and the panic
//! prints the verdict first.

use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use sts2_core::{Player as Lobbyist, RunTable, Seed, Start, Verdict};
use sts2_run::{Carried, Fault, Random, drive, ids::read_id};
use sts2_sim::SimTable;

/// Moves a run takes before this gives up on it.
const STEPS: usize = 20_000;

/// Play one run.
#[derive(Parser)]
struct Cli {
    /// The seed, such as `C000000042`.
    #[arg(value_parser = Seed::new)]
    seed: Seed,
    /// The characters by id, one for each seat, such as `IRONCLAD` or
    /// `IRONCLAD,SILENT`.
    #[arg(value_delimiter = ',', required = true)]
    characters: Vec<String>,
    /// The build to play, as the game names its version.
    #[arg(long, default_value = "v0.107.1")]
    build: String,
    /// What holds the run.
    #[arg(long, value_enum, default_value_t = Table::Sim)]
    table: Table,
    /// What picks every seat's moves.
    #[arg(long, value_enum, default_value_t = Pick::Random)]
    player: Pick,
    /// Every player carries 25 Gorgets and a Mercury Hourglass.
    #[arg(long)]
    carried: bool,
    /// The moves played before the run is called capped.
    #[arg(long, default_value_t = STEPS)]
    steps: usize,
    /// The ascension level, by the id the game spells it with.
    #[arg(long, default_value = "None")]
    ascension: String,
    /// The modifiers the run starts with, by the ids the game spells them
    /// with. Character Cards names its character after a colon, as
    /// `CHARACTER_CARDS:DEFECT`.
    #[arg(long, value_delimiter = ',')]
    modifiers: Vec<String>,
}

/// What holds the run.
#[derive(Clone, Copy, ValueEnum)]
enum Table {
    /// The simulator, one machine's view for each seat.
    Sim,
}

/// What picks the moves.
#[derive(Clone, Copy, ValueEnum)]
enum Pick {
    /// At random, as the census and the recorder pick.
    Random,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("no build {0:?}; try v0.107.1 or v0.111.0")]
    Build(String),
    #[error("{id:?} names nothing: {source}")]
    Id {
        id: String,
        source: serde_json::Error,
    },
}

fn main() -> ExitCode {
    sts2_run::print_verdict_on_panic();
    let cli = Cli::parse();
    // One arm per build `sts2-content` compiles.
    let played = match cli.build.as_str() {
        "v0.107.1" => run::<sts2_content::v0_107_1::V0_107_1>(cli),
        "v0.111.0" => run::<sts2_content::v0_111_0::V0_111_0>(cli),
        other => Err(Error::Build(other.to_owned())),
    };
    match played {
        Ok(verdict) => {
            println!("{verdict}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Content by the id the game spells it with.
fn id<T: serde::de::DeserializeOwned>(id: &str) -> Result<T, Error> {
    read_id(id).map_err(|source| Error::Id {
        id: id.to_owned(),
        source,
    })
}

/// The run `cli` names, played to its verdict.
fn run<V: sts2_content::Build>(cli: Cli) -> Result<Verdict, Error> {
    let Cli {
        seed,
        characters,
        build: _,
        table,
        player,
        carried,
        steps,
        ascension,
        modifiers,
    } = cli;
    let characters = characters
        .iter()
        .map(|character| id::<V::Character>(&character.to_uppercase()))
        .collect::<Result<Vec<_>, _>>()?;
    // A solo run's profile has won as often as its seed says, for the
    // Architect's last line. A co-op lobby's players have won nothing.
    let wins = match characters.len() {
        1 => sts2_run::random::wins(seed.as_str()),
        _ => 0,
    };
    let start = Start {
        players: characters
            .into_iter()
            .map(|character| Lobbyist { character, wins })
            .collect(),
        ascension: id(&ascension)?,
        modifiers: modifiers
            .iter()
            .map(|text| {
                sts2_run::ids::read_modifier(text).map_err(|source| Error::Id {
                    id: text.clone(),
                    source,
                })
            })
            .collect::<Result<_, _>>()?,
        seed,
    };
    let Pick::Random = player;
    let mut player = Random::for_seed(start.seed.as_str());
    let Table::Sim = table;
    Ok(match carried {
        true => play::<V, Carried<SimTable<V>>>((), &start, &mut player, steps),
        false => play::<V, SimTable<V>>((), &start, &mut player, steps),
    })
}

/// The run `start` names at a table of `T`, to its verdict.
fn play<V, T>(host: T::Host, start: &Start<V>, player: &mut Random, steps: usize) -> Verdict
where
    V: sts2_content::Build,
    T: RunTable<V>,
    Fault: From<T::Error>,
{
    match T::begin(host, start) {
        Ok(mut table) => drive(&mut table, player, steps),
        Err(fault) => {
            let fault = Fault::from(fault);
            Verdict {
                end: fault.end,
                step: 0,
                act: 0,
                floor: 0,
                seat: None,
                place: None,
                detail: format!("as the run began: {}", fault.detail),
            }
        }
    }
}
