//! What each seed's first unknown room rolls, for `cargo xtask unknown`.
//!
//! ```text
//! unknown_rolls <character> <first> <count>
//! ```
//!
//! Seeds are the census's, `C` and nine digits. Each prints the seed and the
//! first draw of the `UnknownMapPoint` stream. The draw is read off a copy of
//! the stream, so the run it came from is not moved. Then comes a path to each unknown point
//! of the act's first three rows that a run can reach first. A path is its
//! points from the first row, joined by `>`.
//!
//! After a `|` come the first act's events in the order its unknown rooms pull
//! them, drawn the way a run opens. Neow's room marks one visit against the
//! queue, so the list starts one past its front.

use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::characters::Character;
use sts2_content::v0_107_1::runstate::RunRngType;
use sts2_core::rng::{GameRng as _, stream_seed};
use sts2_core::{ContentVersion, Seed, Start};
use sts2_sim::Sim;
use sts2_sim::rng::MegaRng;
use sts2_sim::run::Opening;

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

/// A path to each unknown point up to `rows` that no earlier unknown point
/// stands in front of, in map order.
fn first_unknowns(points: &[sts2_core::Node<V>], rows: u8) -> Vec<String> {
    let unknown = <<V as ContentVersion>::MapPointType as sts2_core::MapPointKinds>::UNKNOWN;
    let node = |at: sts2_core::Coord| points.iter().find(|node| node.at == at);
    let mut paths: Vec<Vec<sts2_core::Coord>> = points
        .iter()
        .filter(|node| node.at.row == 0)
        .map(|node| vec![node.at])
        .collect();
    let mut found = Vec::new();
    while let Some(path) = paths.pop() {
        let Some(last) = path.last().copied().and_then(node) else {
            continue;
        };
        if last.at.row > 0 && last.kind == unknown {
            let steps: Vec<String> = path[1..]
                .iter()
                .map(|at| format!("{},{}", at.col, at.row))
                .collect();
            found.push((last.at, steps.join(">")));
            continue;
        }
        for &to in last.to.iter().filter(|to| to.row <= rows) {
            let mut longer = path.clone();
            longer.push(to);
            paths.push(longer);
        }
    }
    found.sort_by_key(|(at, _)| (at.row, at.col));
    found.dedup_by_key(|(at, _)| *at);
    found.into_iter().map(|(_, path)| path).collect()
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
            players: vec![sts2_core::Player { character, wins: 0 }],
            ascension: sts2_content::v0_107_1::ascension::AscensionLevel::None,
            modifiers: Vec::new(),
        };
        let sim = |source| Error::Sim {
            seed: seed.clone(),
            source,
        };
        let mut run = Sim::<V>::begin(&start).map_err(sim)?;
        let seen = run.observe().map_err(sim)?;
        let position = seen.observation.rng.run[&RunRngType::UnknownMapPoint];
        let words = position.words.ok_or_else(|| Error::NoWords(seed.clone()))?;
        let counter = i32::try_from(position.counter).map_err(|_| Error::Counter(seed.clone()))?;
        let roll = MegaRng::from_words(words, counter).next_double() as f32;
        let unknown = first_unknowns(&seen.observation.map.points, 3);
        let acts = sts2_sim::run::run_acts::<V>(stream_seed::<V>(&start.seed));
        let mut up_front =
            sts2_core::rng::named::<V, MegaRng>(stream_seed::<V>(&start.seed), "up_front");
        let opening = Opening::<V>::draw(&mut up_front, &[character], &acts, start.ascension);
        let mut queue = opening.acts[0].1.events.clone();
        queue.rotate_left(1);
        let events: Vec<String> = queue
            .iter()
            .map(|event| match serde_json::to_value(event) {
                Ok(serde_json::Value::String(id)) => id,
                _ => format!("{event:?}"),
            })
            .collect();
        println!(
            "{seed} {roll:.4} {} | {}",
            unknown.join(" "),
            events.join(" ")
        );
    }
    Ok(())
}
