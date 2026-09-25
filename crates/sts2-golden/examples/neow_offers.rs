//! What Neow offers on each of a range of seeds, for `cargo xtask neow`.
//!
//! ```text
//! neow_offers <character> <first> <count>
//! ```
//!
//! Seeds are the census's, `C` and nine digits, from `first` for `count` of
//! them. Each prints one line: the seed, then each relic Neow offers in the
//! order the options number them.

use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::characters::Character;
use sts2_core::{Play as _, Room, Seed, Start};
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
    #[error("{0} opens somewhere other than Neow")]
    NotNeow(String),
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
    } = cli;
    for n in first..first + count {
        let seed = format!("C{n:09}");
        let start = Start::<V> {
            seed: Seed::new(&seed).map_err(|source| Error::Seed {
                seed: seed.clone(),
                source,
            })?,
            character,
            ascension: sts2_content::v0_107_1::ascension::AscensionLevel::None,
            modifiers: Vec::new(),
        };
        let sim = |source| Error::Sim {
            seed: seed.clone(),
            source,
        };
        let mut run = Sim::<V>::begin((), &start).map_err(sim)?;
        let seen = run.observe().map_err(sim)?;
        let Some(Room::Event { options, .. }) = seen.observation.room else {
            return Err(Error::NotNeow(seed));
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
