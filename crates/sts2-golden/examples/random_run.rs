//! One run of the simulator at random, for `cargo xtask census`.
//!
//! ```text
//! random_run <seed> <character>
//! ```
//!
//! Every decision is a legal move, picked by a generator seeded from the seed,
//! so a run repeats exactly. Ending the turn is picked one time in ten while
//! anything else is legal. The run carries 25 Gorgets and a Mercury
//! Hourglass, so random play reaches content instead of dying early.
//!
//! It prints `at <act> <floor>` as the floor changes, and `step <n> <move>
//! <moves>` before each decision and `done` after. The last line is `over
//! <won|lost>` or `capped`. An unported path panics, and the census reads the
//! panic off stderr, so nothing here catches one. Anything else that stops a
//! run is an error, which the last line of stderr names as `error: <what>`.

use std::io::Write as _;
use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::characters::Character;
use sts2_content::v0_107_1::relics::Relic;
use sts2_core::{Decision, Gift, Observed as _, Play as _, Seed, Setup, Start};
use sts2_sim::Sim;

type V = V0_107_1;

#[path = "carried/mod.rs"]
mod carried;
use carried::carried;

/// Decisions a run takes before this gives up on it.
const STEPS: usize = 20_000;

/// Play one run at random.
#[derive(Parser)]
struct Cli {
    /// The seed, such as `C000000042`.
    #[arg(value_parser = seed)]
    seed: Seed,
    /// The character, by id, such as `IRONCLAD`.
    #[arg(value_parser = character)]
    character: Character,
    /// The ascension level, by the id the game spells it with.
    #[arg(long, value_parser = ascension, default_value = "None")]
    ascension: sts2_content::v0_107_1::ascension::AscensionLevel,
    /// The modifiers the run starts with, by the ids the game spells them
    /// with. Character Cards names its character after a colon, as
    /// `CHARACTER_CARDS:DEFECT`.
    #[arg(long, value_parser = modifier, value_delimiter = ',')]
    modifiers: Vec<sts2_core::RunModifier<V>>,
}

/// A modifier as [`sts2_golden::recorded::spell_modifier`] writes it.
fn modifier(text: &str) -> Result<sts2_core::RunModifier<V>, serde_json::Error> {
    sts2_golden::recorded::read_modifier(text)
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
    #[error("starting the run: {0}")]
    Begin(#[source] sts2_sim::Error),
    #[error("granting {relic:?}: {source}")]
    Grant {
        relic: Relic,
        #[source]
        source: sts2_sim::Error,
    },
    #[error("the run refused {relic:?}: {refusal}")]
    GrantRefused {
        relic: Relic,
        refusal: sts2_core::Refusal<V>,
    },
    #[error("a run with nothing legal to do")]
    NothingLegal,
    #[error("step {step}: {source}")]
    Decide {
        step: usize,
        #[source]
        source: sts2_sim::Error,
    },
    #[error("step {step}: the run refused its own legal move {decision:?}: {refusal}")]
    Refused {
        step: usize,
        decision: Decision<V>,
        refusal: sts2_core::Refusal<V>,
    },
    #[error("writing to stdout: {0}")]
    Stdout(#[from] std::io::Error),
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Error> {
    let Cli {
        seed,
        character,
        ascension,
        modifiers,
    } = cli;
    let start = Start::<V> {
        wins: sts2_golden::random::wins(seed.as_str()),
        seed,
        character,
        ascension,
        modifiers,
    };
    let mut pick = sts2_golden::random::Pick::for_seed(start.seed.as_str());
    let mut sim = Sim::<V>::begin((), &start).map_err(Error::Begin)?;
    for (relic, count) in carried::<V>() {
        for _ in 0..count {
            sim.construct(&Setup::Grant(Gift::Relic(relic)))
                .map_err(|source| Error::Grant { relic, source })?
                .map_err(|refusal| Error::GrantRefused { relic, refusal })?;
        }
    }
    let mut out = std::io::stdout().lock();
    let mut last = None;

    for step in 0..STEPS {
        let run = sim.observed();
        let here = (run.act(), run.floor());
        if last != Some(here) {
            writeln!(out, "at {} {}", here.0, here.1)?;
            last = Some(here);
        }
        if run.run_over() {
            let how = match run.victory() {
                Some(true) => "won",
                _ => "lost",
            };
            writeln!(out, "over {how}")?;
            return Ok(());
        }
        let legal = sim.legal();
        if legal.is_empty() {
            return Err(Error::NothingLegal);
        }
        // The question is built only where a discard could make room, which
        // a full belt on a reward screen is.
        let (held, slots) = (run.potions().len(), run.potion_slots());
        let full = num_traits::ToPrimitive::to_usize(&slots).is_some_and(|slots| held >= slots);
        let room = full
            && legal
                .iter()
                .any(|decision| matches!(decision, Decision::DiscardPotion { .. }))
            && {
                let asking = sim.observe().map(|seen| seen.asking).unwrap_or(None);
                sts2_golden::random::making_room(asking.as_ref(), held, slots)
            };
        let at = pick.choose(&legal, room);
        writeln!(out, "step {step} {at} {}", legal.len())?;
        out.flush()?;
        let decision = legal[at];
        sim.decide(&decision)
            .map_err(|source| Error::Decide { step, source })?
            .map_err(|refusal| Error::Refused {
                step,
                decision,
                refusal,
            })?;
        writeln!(out, "done")?;
    }
    writeln!(out, "capped")?;
    Ok(())
}
