//! Runs the game's dump modes and files what they print.
//! Each mode is one launch of the real game with `STS2_HEADLESS_MODE` set. Each prints its table to stdout, between `[<mode>] begin` and `[<mode>] end` markers.
//! The mod prints instead of writing files. A mod that writes into the game directory is a worse habit than a caller that extracts. This is that caller.
//! Most modes describe content. They land beside each other under the version directory, where [`crate::codegen`] reads them. Two are ground truth for tests, which is not content and lands beside those tests.
//! Three modes launch nothing. `cardplaydump` and `monstermovedump` read what cards and monsters do off the decompiled assembly, which [`crate::plays`] explains. `textdump` copies the English text tables out of the build's pack.
//! Nothing here checks that two dumps came from the same build. Codegen refuses to generate from dumps whose content hashes disagree. That catches a half-finished re-dump before it can do damage.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{self, FileError, ProcessError};

use crate::codegen::version_dir;

/// Where a mode's output belongs.
enum Destination {
    /// Beside the other content dumps, named by file.
    Content(&'static str),
    /// Somewhere else entirely, named from the repository root: a test
    /// fixture taken from v0.107.1, which a dump of another build leaves.
    At(&'static str),
    /// Beside the content dumps, read off the decompiled assembly rather than
    /// printed by the game. The reader, then what it says of the table it read.
    Decompiled(&'static str, Reader, fn(&str) -> String),
    /// Beside the content dumps under `text/`: the English text tables, read
    /// out of the build's pack rather than printed by the game.
    Packed(&'static [&'static str]),
}

/// A reader of the decompiled assembly: the game directory, the build it has
/// to hold, and where that build's dumps live.
type Reader = fn(&Path, &str, &Path) -> Result<String, crate::plays::Error>;

use Destination::{At, Content, Decompiled, Packed};

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
    // After the card and power dumps, whose ids it names.
    (
        "cardplaydump",
        Decompiled(
            "card_plays.txt",
            crate::plays::cards::cards,
            crate::plays::cards::tally,
        ),
    ),
    // After the monster dump, whose moves it reads.
    (
        "monstermovedump",
        Decompiled(
            "monster_moves.txt",
            crate::plays::monsters::monsters,
            crate::plays::monsters::tally,
        ),
    ),
    ("textdump", Packed(TEXT)),
    ("rngdump", At("crates/sts2-sim/tests/rng/megarandom.txt")),
    ("setupdump", At("crates/sts2-golden/tests/setup.txt")),
];

/// The English text tables a front end draws from: what each card, relic,
/// power, potion and enchantment says, and the words around them.
const TEXT: &[&str] = &[
    "afflictions",
    "ancients",
    "card_keywords",
    "cards",
    "characters",
    "enchantments",
    "events",
    "intents",
    "monsters",
    "orbs",
    "potions",
    "powers",
    "relics",
    "static_hover_tips",
];

#[derive(clap::Args)]
pub struct Args {
    /// The modes to dump, such as `carddump`. Every one when none is named.
    #[arg(value_parser = clap::builder::PossibleValuesParser::new(DUMPS.iter().map(|(mode, _)| *mode)))]
    modes: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Version(#[from] crate::codegen::VersionError),
    #[error(transparent)]
    Plays(#[from] crate::plays::Error),
    #[error(transparent)]
    Pack(#[from] crate::pck::Error),
    #[error(
        "STS2_GAME_DIR is not set. The repo .envrc exports it: run `direnv allow` in the repo root, or set it by hand"
    )]
    NoGameDir,
    #[error("{mode}: {source}")]
    Launch {
        mode: &'static str,
        #[source]
        source: ProcessError,
    },
    #[error("{mode}: the game printed non-UTF-8: {source}")]
    NotText {
        mode: &'static str,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("{mode}: printed no '[{mode}] begin'. Are the mods deployed?")]
    NoBegin { mode: &'static str },
    #[error("{mode}: printed '[{mode}] begin' but never '[{mode}] end'")]
    NoEnd { mode: &'static str },
    #[error("{mode}: printed an empty table")]
    EmptyTable { mode: &'static str },
}

/// Run every mode named, or all of them when none is.
pub fn run(args: Args) -> Result<String, Error> {
    let modes = args.modes;
    let game = PathBuf::from(std::env::var("STS2_GAME_DIR").map_err(|_| Error::NoGameDir)?);

    let wanted = select(&modes);
    let mut done = Vec::new();

    for (mode, destination) in wanted {
        if let Packed(tables) = destination {
            let mut pack = crate::pck::Pack::open(&game.join("SlayTheSpire2.pck"))?;
            let dir = version_dir()?.join("dumps").join("text");
            error::create_dir(&dir)?;
            for table in *tables {
                let text = pack.text(&format!("localization/eng/{table}.json"))?;
                error::write(&dir.join(format!("{table}.json")), &text)?;
            }
            println!("{mode} -> {} ({} tables)", dir.display(), tables.len());
            done.push(mode);
            continue;
        }
        let (table, path) = match destination {
            Decompiled(name, read, tally) => {
                let table = read(&game, &crate::codegen::version()?, &version_dir()?)?;
                println!("{mode}: {}", tally(&table));
                (table, version_dir()?.join("dumps").join(name))
            }
            Content(name) => (
                extract(&launch(&game, mode)?, mode)?,
                version_dir()?.join("dumps").join(name),
            ),
            Packed(_) => unreachable!("handled above"),
            At(path) => {
                let version = crate::codegen::version()?;
                if version != "v0.107.1" {
                    println!("{mode}: {path} is v0.107.1's, so {version} leaves it");
                    continue;
                }
                (extract(&launch(&game, mode)?, mode)?, PathBuf::from(path))
            }
        };
        error::write(&path, &table)?;
        println!("{mode} -> {}", path.display());
        done.push(mode);
    }

    Ok(format!("dumped {} of {}", done.len(), DUMPS.len()))
}

/// The requested modes, or all of them. Clap has refused any name that is not
/// one.
fn select(modes: &[String]) -> Vec<&'static (&'static str, Destination)> {
    if modes.is_empty() {
        return DUMPS.iter().collect();
    }
    DUMPS
        .iter()
        .filter(|(mode, _)| modes.iter().any(|wanted| wanted == mode))
        .collect()
}

/// One launch of the real game, headless, with the mode set.
///
/// Through `timeout -k`, not a bare `timeout`, for the reason the justfile
/// gives. Without `-k` this sends a SIGTERM that an un-armed instance ignores.
///
/// `LD_PRELOAD` is not optional either. MonoMod's native helper imports
/// `_Unwind_*` from libgcc and declares no dependency on it, and the game does
/// not link libgcc. Without the preload every Harmony patch throws.
fn launch(game: &Path, mode: &'static str) -> Result<String, Error> {
    let out = error::output(
        Command::new("timeout")
            .args(["-k", "5", "300"])
            .arg("env")
            .arg("LD_PRELOAD=libgcc_s.so.1")
            .arg(format!("STS2_HEADLESS_MODE={mode}"))
            .arg("./SlayTheSpire2")
            .args(["--headless", "--force-steam=off"])
            .current_dir(game),
    )
    .map_err(|source| Error::Launch { mode, source })?;

    // The dump goes to stdout in full. stderr carries Godot's shutdown grumbling and nothing we want.
    String::from_utf8(out.stdout).map_err(|source| Error::NotText { mode, source })
}

/// The table between the markers.
///
/// Matched on the end of the line, because the log prefixes its level. A mode that ran prints `[INFO] [carddump] begin`.
///
/// The game also logs the mode name when it *declines* one. A looser match can find that line instead, and then produce nothing at all.
fn extract(log: &str, mode: &'static str) -> Result<String, Error> {
    let begin = format!("[{mode}] begin");
    let end = format!("[{mode}] end");

    let after = log
        .lines()
        .position(|l| l.ends_with(&begin))
        .ok_or(Error::NoBegin { mode })?;
    let before = log
        .lines()
        .skip(after)
        .position(|l| l.ends_with(&end))
        .ok_or(Error::NoEnd { mode })?;

    let mut table: Vec<&str> = log.lines().skip(after + 1).take(before - 1).collect();

    // The mod appends the table to the begin marker, and the table ends in a newline, so there is always exactly one blank line to give back.
    if table.last() == Some(&"") {
        table.pop();
    }
    if table.is_empty() {
        return Err(Error::EmptyTable { mode });
    }

    let mut text = table.join("\n");
    text.push('\n');
    Ok(text)
}
