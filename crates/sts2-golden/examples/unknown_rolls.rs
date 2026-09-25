//! What each seed's first unknown room rolls, for `cargo xtask unknown`.
//!
//! ```text
//! unknown_rolls <character> <first> <count>
//! ```
//!
//! Seeds are the census's, `C` and nine digits. Each prints the seed, the
//! first draw of the `UnknownMapPoint` stream, and every unknown point of the
//! act's first three rows. The draw is read off a copy of the stream, so the
//! run it came from is not moved.

use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::characters::Character;
use sts2_content::v0_107_1::runstate::RunRngType;
use sts2_core::rng::GameRng as _;
use sts2_core::{Play as _, Seed, Start};
use sts2_sim::Sim;
use sts2_sim::rng::MegaRng;

type V = V0_107_1;

/// Print the first unknown room's draw for each of a range of census seeds.
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
    #[error("{0}: the report carries the UnknownMapPoint stream's count and not its words")]
    NoWords(String),
    #[error("{0}: the UnknownMapPoint stream has drawn past what its counter holds")]
    Counter(String),
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
        let position = seen.observation.rng.run[&RunRngType::UnknownMapPoint];
        let words = position.words.ok_or_else(|| Error::NoWords(seed.clone()))?;
        let counter = i32::try_from(position.counter).map_err(|_| Error::Counter(seed.clone()))?;
        let roll = MegaRng::from_words(words, counter).next_double() as f32;
        let unknown: Vec<String> = seen
            .observation
            .map
            .points
            .iter()
            .filter(|node| {
                node.at.row <= 3 && node.kind == <_ as sts2_core::MapPointKinds>::UNKNOWN
            })
            .map(|node| format!("{},{}", node.at.col, node.at.row))
            .collect();
        println!("{seed} {roll:.4} {}", unknown.join(" "));
    }
    Ok(())
}
