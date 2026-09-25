//! `cargo xtask neow <RELIC> [--character <C>] [--seeds <N>]`: the census
//! seeds whose Neow offers a relic, and the option that takes it.
//!
//! A scenario that picks a relic up at Neow needs a seed that offers it. This
//! runs the `neow_offers` example over the seeds and keeps the ones that name
//! the relic. The simulator answers, so a relic it cannot offer yet is never
//! found.

use std::process::Command;

use crate::instructions::workspace;

pub fn run(args: &[String]) -> Result<String, String> {
    let mut relic = None;
    let mut character = "IRONCLAD".to_string();
    let mut seeds = 300u64;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--character" => character = args.next().ok_or("--character wants a value")?.clone(),
            "--seeds" => {
                seeds = args
                    .next()
                    .ok_or("--seeds wants a value")?
                    .parse()
                    .map_err(|e| format!("--seeds: {e}"))?;
            }
            other if relic.is_none() => relic = Some(other.to_string()),
            other => return Err(format!("unexpected argument {other}")),
        }
    }
    let relic = relic.ok_or("usage: cargo xtask neow <RELIC> [--character C] [--seeds N]")?;

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
