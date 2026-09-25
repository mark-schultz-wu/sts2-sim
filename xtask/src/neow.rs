//! `cargo xtask neow <RELIC> [--character <C>] [--seeds <N>]`: the census
//! seeds whose Neow offers a relic, and the option that takes it.
//!
//! A scenario that picks a relic up at Neow needs a seed that offers it. This
//! runs the `neow_offers` example over the seeds and keeps the ones that name
//! the relic. The simulator answers, so a relic it cannot offer yet is never
//! found.

use std::process::Command;

use crate::instructions::workspace;

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

pub fn run(args: Args) -> Result<String, String> {
    let Args {
        relic,
        character,
        seeds,
    } = args;

    let output = Command::new("cargo")
        .current_dir(workspace()?)
        .args([
            "run",
            "-q",
            "-p",
            "sts2-golden",
            "--example",
            "neow_offers",
            "--",
        ])
        .args([character.as_str(), "0", &seeds.to_string()])
        .output()
        .map_err(|e| format!("running neow_offers: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let mut found = String::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut words = line.split_whitespace();
        let Some(seed) = words.next() else { continue };
        if let Some(at) = words.position(|offered| offered == relic) {
            found.push_str(&format!("{seed} {character} option {at}\n"));
        }
    }
    if found.is_empty() {
        return Err(format!("no seed of {seeds} offers {relic} to {character}"));
    }
    Ok(found)
}
