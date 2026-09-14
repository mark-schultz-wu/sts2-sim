//! Runs the game's dump modes and files what they print.
//! Each mode is one launch of the real game with `STS2_HEADLESS_MODE` set. Each prints its table to stdout, between `[<mode>] begin` and `[<mode>] end` markers.
//! The mod prints instead of writing files. A mod that writes into the game directory is a worse habit than a caller that extracts. This is that caller.
//! Fifteen modes describe content. They land beside each other under the version directory, where [`crate::codegen`] reads them. The sixteenth is rng ground truth, which is not content and does not.
//! Nothing here checks that two dumps came from the same build. Codegen refuses to generate from dumps whose content hashes disagree. That catches a half-finished re-dump before it can do damage.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::codegen::{version_dir, version_module};

/// Where a mode's output belongs.
enum Destination {
    /// Beside the other content dumps, named by file.
    Content(&'static str),
    /// Somewhere else entirely, named from the repository root.
    At(&'static str),
}

use Destination::{At, Content};

/// Every mode the dump mod answers to. The marker it prints is its own name, so only the destination is worth writing down.
const DUMPS: &[(&str, Destination)] = &[
    ("actdump", Content("acts.txt")),
    ("afflictiondump", Content("afflictions.txt")),
    ("ascensiondump", Content("ascension.txt")),
    ("carddump", Content("cards.txt")),
    ("characterdump", Content("characters.txt")),
    ("enchantmentdump", Content("enchantments.txt")),
    ("encounterdump", Content("encounters.txt")),
    ("eventdump", Content("events.txt")),
    ("hookdump", Content("hooks.txt")),
    ("modifierdump", Content("modifiers.txt")),
    ("monsterdump", Content("monsters.txt")),
    ("orbdump", Content("orbs.txt")),
    ("potiondump", Content("potions.txt")),
    ("powerdump", Content("powers.txt")),
    ("relicdump", Content("relics.txt")),
    ("restsitedump", Content("restsite.txt")),
    ("runstatedump", Content("runstate.txt")),
    ("rngdump", At("crates/sts2-sim/tests/rng/megarandom.txt")),
    ("setupdump", At("crates/sts2-golden/tests/setup.txt")),
];

/// Run every mode named, or all of them when none is.
pub fn run(modes: &[String]) -> Result<String, String> {
    let game = std::env::var("STS2_GAME_DIR").map_err(|_| {
        "STS2_GAME_DIR is not set. The repo .envrc exports it: run `direnv allow` in the repo \
         root, or set it by hand"
            .to_string()
    })?;
    let game = PathBuf::from(game);

    let wanted = select(modes)?;
    let mut done = Vec::new();

    for (mode, destination) in wanted {
        let log = launch(&game, mode)?;
        let table = extract(&log, mode)?;
        let path = match destination {
            Content(name) => version_dir()?.join("dumps").join(name),
            At(path) => PathBuf::from(path.replace("{version}", &version_module()?)),
        };
        fs::write(&path, &table).map_err(|e| format!("writing {}: {e}", path.display()))?;
        println!("{mode} -> {}", path.display());
        done.push(mode);
    }

    Ok(format!("dumped {} of {}", done.len(), DUMPS.len()))
}

/// The requested modes, or all of them. An unknown name is a typo worth stopping for, not a mode to skip.
fn select(modes: &[String]) -> Result<Vec<&'static (&'static str, Destination)>, String> {
    if modes.is_empty() {
        return Ok(DUMPS.iter().collect());
    }
    modes
        .iter()
        .map(|wanted| {
            DUMPS
                .iter()
                .find(|(mode, _)| mode == wanted)
                .ok_or_else(|| format!("unknown dump mode '{wanted}'; try one of: {}", names()))
        })
        .collect()
}

fn names() -> String {
    DUMPS
        .iter()
        .map(|(mode, _)| *mode)
        .collect::<Vec<_>>()
        .join(", ")
}

/// One launch of the real game, headless, with the mode set.
///
/// Through `timeout -k`, not a bare `timeout`, for the reason the justfile
/// gives. Without `-k` this sends a SIGTERM that an un-armed instance ignores.
///
/// `LD_PRELOAD` is not optional either. MonoMod's native helper imports
/// `_Unwind_*` from libgcc and declares no dependency on it, and the game does
/// not link libgcc. Without the preload every Harmony patch throws.
fn launch(game: &Path, mode: &str) -> Result<String, String> {
    let out = Command::new("timeout")
        .args(["-k", "5", "300"])
        .arg("env")
        .arg("LD_PRELOAD=libgcc_s.so.1")
        .arg(format!("STS2_HEADLESS_MODE={mode}"))
        .arg("./SlayTheSpire2")
        .args(["--headless", "--force-steam=off"])
        .current_dir(game)
        .output()
        .map_err(|e| format!("launching the game in {}: {e}", game.display()))?;

    if !out.status.success() {
        return Err(format!(
            "{mode}: the game exited {}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    // The dump goes to stdout in full. stderr carries Godot's shutdown grumbling and nothing we want.
    String::from_utf8(out.stdout).map_err(|e| format!("{mode}: the game printed non-UTF-8: {e}"))
}

/// The table between the markers.
///
/// Matched on the end of the line, because the log prefixes its level. A mode that ran prints `[INFO] [carddump] begin`.
///
/// The game also logs the mode name when it *declines* one. A looser match can find that line instead, and then produce nothing at all.
fn extract(log: &str, mode: &str) -> Result<String, String> {
    let begin = format!("[{mode}] begin");
    let end = format!("[{mode}] end");

    let after = log
        .lines()
        .position(|l| l.ends_with(&begin))
        .ok_or_else(|| format!("{mode}: printed no '{begin}'. Are the mods deployed?"))?;
    let before = log
        .lines()
        .skip(after)
        .position(|l| l.ends_with(&end))
        .ok_or_else(|| format!("{mode}: printed '{begin}' but never '{end}'"))?;

    let mut table: Vec<&str> = log.lines().skip(after + 1).take(before - 1).collect();

    // The mod appends the table to the begin marker, and the table ends in a newline, so there is always exactly one blank line to give back.
    if table.last() == Some(&"") {
        table.pop();
    }
    if table.is_empty() {
        return Err(format!("{mode}: printed an empty table"));
    }

    let mut text = table.join("\n");
    text.push('\n');
    Ok(text)
}
