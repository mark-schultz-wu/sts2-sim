//! What Neow offers on each of a range of seeds, for `cargo xtask neow`, or
//! another ancient entered by name.
//!
//! ```text
//! neow_offers <character> <first> <count> [--event <EVENT>]
//! ```
//!
//! Seeds are the census's, `C` and nine digits, from `first` for `count` of
//! them. Each prints one line: the seed, then each relic the event offers in
//! the order the options number them. A named event is entered the way a
//! scenario's `event` step enters it.

use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::characters::Character;
use sts2_content::v0_107_1::events::Event;
use sts2_core::{Play as _, Room, Seed, Setup, Start};
use sts2_sim::Sim;

type V = V0_107_1;

/// Print Neow's offer for each of a range of census seeds.
#[derive(Parser)]
struct Cli {
    /// The character, by id, such as `IRONCLAD`.
    #[arg(value_parser = character)]
    character: Character,
    /// The first seed's number.
    first: u64,
    /// How many seeds.
    count: u64,
    /// An ancient to enter by name, such as `OROBAS`, rather than Neow.
    #[arg(long, value_parser = event)]
    event: Option<Event>,
}

/// An event by the id the game spells it with.
fn event(id: &str) -> Result<Event, serde_json::Error> {
    serde_json::from_value(serde_json::Value::String(id.to_owned()))
}

/// A character by the id the game spells it with.
fn character(id: &str) -> Result<Character, serde_json::Error> {
    serde_json::from_value(serde_json::Value::String(id.to_owned()))
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("{seed}: {source}")]
    Seed {
        seed: String,
        #[source]
        source: sts2_core::seed::Error,
    },
    #[error("{seed}: {source}")]
    Sim {
        seed: String,
        #[source]
        source: sts2_sim::Error,
    },
    #[error("{seed}: the event was refused: {refusal:?}")]
    Refused {
        seed: String,
        refusal: sts2_core::Refusal<V>,
    },
    #[error("{0} stands somewhere other than an event")]
    NotEvent(String),
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Error> {
    let Cli {
        character,
        first,
        count,
        event,
    } = cli;
    for n in first..first + count {
        let seed = format!("C{n:09}");
        let start = Start::<V> {
            seed: Seed::new(&seed).map_err(|source| Error::Seed {
                seed: seed.clone(),
                source,
            })?,
            players: vec![sts2_core::Player { character, wins: 0 }],
            ascension: sts2_content::v0_107_1::ascension::AscensionLevel::None,
            modifiers: Vec::new(),
        };
        let sim = |source| Error::Sim {
            seed: seed.clone(),
            source,
        };
        let mut run = Sim::<V>::begin((), &start).map_err(sim)?;
        if let Some(event) = event
            && let Err(refusal) = run.construct(&Setup::Event { event }).map_err(sim)?
        {
            return Err(Error::Refused { seed, refusal });
        }
        let seen = run.observe().map_err(sim)?;
        let Some(Room::Event { options, .. }) = seen.observation.room else {
            return Err(Error::NotEvent(seed));
        };
        let offered: Vec<String> = options
            .iter()
            .map(|option| match option.relic {
                Some(relic) => serde_json::to_value(relic)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| format!("{relic:?}")),
                None => "-".to_string(),
            })
            .collect();
        println!("{seed} {}", offered.join(" "));
    }
    Ok(())
}
