//! `cargo xtask neow <RELIC> [--character <C>] [--seeds <N>]`: the census
//! seeds whose Neow offers a relic, and the option that takes it.
//!
//! A scenario that picks a relic up at Neow needs a seed that offers it. This
//! runs the `neow_offers` example over the seeds and keeps the ones that name
//! the relic. The simulator answers, so a relic it cannot offer yet is never
//! found.

use std::process::Command;

#[derive(clap::Args)]
pub struct Args {
    /// The relic, by id, such as `NEW_LEAF`.
    relic: String,
    /// The character whose Neow is asked.
    #[arg(long, default_value = "IRONCLAD")]
    character: String,
    /// How many seeds to try, from the census's first.
    #[arg(long, default_value_t = 300)]
    seeds: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Process(#[from] crate::error::ProcessError),
    #[error("no seed of {seeds} offers {relic} to {character}")]
    NotOffered {
        relic: String,
        character: String,
        seeds: u64,
    },
}

pub fn run(args: Args) -> Result<String, Error> {
    let Args {
        relic,
        character,
        seeds,
    } = args;

    let output = crate::error::output(
        Command::new("cargo")
            .current_dir(crate::workspace())
            .args([
                "run",
                "-q",
                "-p",
                "sts2-golden",
                "--example",
                "neow_offers",
                "--",
            ])
            .args([character.as_str(), "0", &seeds.to_string()]),
    )?;
    let mut found = String::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut words = line.split_whitespace();
        let Some(seed) = words.next() else { continue };
        if let Some(at) = words.position(|offered| offered == relic) {
            found.push_str(&format!("{seed} {character} option {at}\n"));
        }
    }
    if found.is_empty() {
        return Err(Error::NotOffered {
            relic,
            character,
            seeds,
        });
    }
    Ok(found)
}
