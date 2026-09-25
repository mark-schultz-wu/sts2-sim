//! How long each list in a `Playable` gets, across the recorded runs.
//!
//! `legal::hand` builds one `Playable` per card in hand on every report, and
//! each carries three heap lists. A fixed cap needs a measurement, so this
//! replays every recording and prints what the lists reach.

use std::process::ExitCode;

use clap::Parser;
use std::collections::BTreeMap;

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Play, Step};
use sts2_golden::bench::reached;
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

/// How long each of a report's lists gets, across the recorded runs.
#[derive(Parser)]
struct Cli {}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Golden(#[from] sts2_golden::Error),
    #[error("{run}: starting the run: {source}")]
    Begin {
        run: String,
        #[source]
        source: sts2_sim::Error,
    },
    #[error("{run}: step {at}: {source}")]
    Step {
        run: String,
        at: usize,
        #[source]
        source: sts2_sim::Error,
    },
    #[error("{run}: step {at} refused: {refusal}")]
    Refused {
        run: String,
        at: usize,
        refusal: sts2_core::Refusal<V>,
    },
}

fn main() -> ExitCode {
    let Cli {} = Cli::parse();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Error> {
    let mut targets: BTreeMap<usize, usize> = BTreeMap::new();
    let mut effect: BTreeMap<usize, usize> = BTreeMap::new();
    let mut reasons: BTreeMap<u32, usize> = BTreeMap::new();
    let mut creatures: BTreeMap<usize, usize> = BTreeMap::new();
    let mut hand: BTreeMap<usize, usize> = BTreeMap::new();
    let mut belt: BTreeMap<usize, usize> = BTreeMap::new();

    for name in every_run()? {
        let recorded = recording::<V>(&name)?;
        let start = recorded.start().clone();
        let steps: Vec<Step<V>> = recorded.steps().to_vec();
        let supported = reached::<V, Sim<V>>(&start, &steps);
        let mut playing = Sim::<V>::begin((), &start).map_err(|source| Error::Begin {
            run: name.clone(),
            source,
        })?;
        for (at, step) in steps[..supported].iter().enumerate() {
            let seen = playing
                .step(step)
                .map_err(|source| Error::Step {
                    run: name.clone(),
                    at,
                    source,
                })?
                .map_err(|refusal| Error::Refused {
                    run: name.clone(),
                    at,
                    refusal,
                })?;
            for held in &seen.observation.potions {
                *belt.entry(held.targets.len()).or_default() += 1;
            }
            let Some(combat) = seen.observation.combat.as_ref() else {
                continue;
            };
            *creatures.entry(combat.creatures.len()).or_default() += 1;
            *hand.entry(combat.legal.len()).or_default() += 1;
            for card in &combat.legal {
                *targets.entry(card.targets.len()).or_default() += 1;
                *effect.entry(card.effect.len()).or_default() += 1;
                let set = card.reason.map_or(0, |reason| {
                    use sts2_core::FlagSet;
                    <<V as sts2_core::ContentVersion>::UnplayableReasons as FlagSet>::MEMBERS
                        .iter()
                        .filter(|&&member| reason.contains(member))
                        .count() as u32
                });
                *reasons.entry(set).or_default() += 1;
            }
        }
    }

    for (what, counts) in [
        ("creatures in a fight", &creatures),
        ("cards in hand", &hand),
        ("targets per card", &targets),
        ("previews per card", &effect),
        ("targets per potion", &belt),
    ] {
        let total: usize = counts.values().sum();
        let line: Vec<String> = counts
            .iter()
            .map(|(len, hits)| format!("{len}:{}%", hits * 100 / total.max(1)))
            .collect();
        println!("{what:22} {}", line.join("  "));
    }
    let total: usize = reasons.values().sum();
    let line: Vec<String> = reasons
        .iter()
        .map(|(len, hits)| format!("{len}:{}%", hits * 100 / total.max(1)))
        .collect();
    println!("{:22} {}", "reasons per card", line.join("  "));
    Ok(())
}
