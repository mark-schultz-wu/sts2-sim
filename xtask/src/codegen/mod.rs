//! Turns the dumps in `dumps/` into Rust, one module per kind of content.
//!
//! Each domain is a submodule here and a file under `generated/`. A domain
//! reads exactly one dump and writes exactly one tree, so adding relics means
//! adding a dump, a submodule, and a line in [`DOMAINS`] — nothing existing has
//! to move.
//!
//! Every dump carries the game's content hash in its header, and [`run`]
//! refuses to generate when two of them disagree: that would mean the tables in
//! one crate came from two different builds, which is exactly the kind of drift
//! this whole arrangement exists to make impossible.
//!
//! The generated tree is committed rather than produced by a `build.rs`, so a
//! content change reads as a diff. In a project about noticing when the game
//! and the simulator disagree, "which forty cards did that patch touch" is
//! worth being able to see.

mod acts;
mod cards;
mod characters;
mod enchantments;
mod encounters;
mod events;
mod monsters;
mod potions;
mod powers;
mod relics;
mod sets;
mod split;

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const VERSION_DIR: &str = "crates/sts2-content/src/v0_107_1";

/// The domains generated so far. Relics, potions and encounters each get a
/// dump, a submodule, and an entry here.
const DOMAINS: [&str; 10] = [
    "acts",
    "cards",
    "characters",
    "enchantments",
    "encounters",
    "events",
    "monsters",
    "potions",
    "powers",
    "relics",
];

const DERIVES: &str = "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]";

/// Rust identifiers the game's names collide with.
///
/// `Self` is the only one that bites in practice, and it cannot be escaped as
/// `r#Self` either — the language reserves it outright. The serde rename keeps
/// the game's spelling on the wire, so this is a Rust-side name only.
fn sanitize(name: &str) -> &str {
    match name {
        "Self" => "SelfTarget",
        other => other,
    }
}

/// Ids are `SCREAMING_SNAKE`; variants are `CamelCase`.
fn camel(id: &str) -> String {
    id.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect()
}

/// The narrowest discriminant that holds this many variants.
///
/// Layout only: serde goes through the variant rename, so nothing on the wire
/// or in a dump depends on the number a variant compiles to. It is still worth
/// choosing rather than defaulting to the wide one, because a discriminant sits
/// next to other fields — an encounter's `Spawn` is a `Monster` beside a slot
/// index, which is two bytes when `Monster` is a `u8` and four when it is a
/// `u16`, and it appears four times in every roster.
///
/// So the domains differ: 88 cards in the largest pool and 126 monsters fit a
/// byte, and 273 powers do not.
fn repr(variants: usize) -> Result<&'static str, String> {
    match variants {
        n if n <= u8::MAX as usize + 1 => Ok("u8"),
        n if n <= u16::MAX as usize + 1 => Ok("u16"),
        n => Err(format!(
            "{n} variants is past what even a u16 discriminant holds"
        )),
    }
}

/// A dump, split into whitespace-separated rows, with its content hash.
///
/// Comment lines are dropped except the header, which every dump opens with and
/// which ends in the hash.
struct Dump {
    hash: String,
    rows: Vec<Vec<String>>,
}

impl Dump {
    fn read(name: &str) -> Result<Dump, String> {
        let path = Path::new(VERSION_DIR).join("dumps").join(name);
        let text =
            fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;

        let header = text
            .lines()
            .next()
            .ok_or_else(|| format!("{name} is empty"))?;
        let hash = header
            .rsplit_once(' ')
            .ok_or_else(|| format!("{name}'s header names no content hash: {header}"))?
            .1
            .to_string();

        if hash.parse::<u32>().is_err() {
            return Err(format!("{name}'s content hash is not a number: {hash}"));
        }

        let rows = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| l.split_whitespace().map(String::from).collect())
            .collect();

        Ok(Dump { hash, rows })
    }

    /// Every row of one kind, in dump order — which is load-bearing for rows
    /// whose position is their index.
    fn of(&self, kind: &str) -> impl Iterator<Item = &Vec<String>> {
        self.rows.iter().filter(move |r| r[0] == kind)
    }
}

pub fn run() -> Result<String, String> {
    let root = Path::new(VERSION_DIR).join("generated");
    fs::create_dir_all(&root).map_err(|e| format!("creating {}: {e}", root.display()))?;

    let hashes = [
        acts::generate(&root)?,
        cards::generate(&root)?,
        characters::generate(&root)?,
        enchantments::generate(&root)?,
        encounters::generate(&root)?,
        events::generate(&root)?,
        monsters::generate(&root)?,
        potions::generate(&root)?,
        powers::generate(&root)?,
        relics::generate(&root)?,
    ];

    // Two dumps from two builds would produce a crate whose tables describe
    // neither. Cheap to check, and impossible to notice by eye.
    let hash = &hashes[0];
    for (domain, h) in DOMAINS.iter().zip(&hashes) {
        if h != hash {
            return Err(format!(
                "{domain}'s dump has content hash {h}, but {} has {hash} — they came from different builds",
                DOMAINS[0]
            ));
        }
    }

    // The roll-up. One line per domain, so adding relics is adding a dump, a
    // module, and an entry in `DOMAINS`.
    let mut out = format!(
        "//! Generated by `cargo xtask codegen`. DO NOT EDIT.\n//!\n//! One module per kind of content.\n//! Source: `dumps/`, content hash {hash}.\n\n"
    );
    for domain in DOMAINS {
        writeln!(out, "pub mod {domain};").unwrap();
    }
    let path = root.join("mod.rs");
    fs::write(&path, &out).map_err(|e| format!("writing {}: {e}", path.display()))?;

    // Formatted rather than emitted pre-formatted. `just lint` runs
    // `cargo fmt --all --check` over the whole workspace, generated files
    // included, and matching rustfmt by hand in a `writeln!` is a losing game —
    // it reorders `mod` declarations and rewraps anything past the margin.
    format(&root)?;

    Ok(format!(
        "generated {VERSION_DIR}/generated/ — {} domains: {}\n  content hash {hash}",
        DOMAINS.len(),
        DOMAINS.join(", ")
    ))
}

/// The banner every generated file opens with.
fn banner(what: &str, source: &str, hash: &str) -> String {
    format!(
        "//! Generated by `cargo xtask codegen`. DO NOT EDIT.\n//!\n//! {what}\n//! Source: `dumps/{source}`, content hash {hash}.\n"
    )
}

/// Runs rustfmt over everything just written.
fn format(root: &Path) -> Result<(), String> {
    let mut files: Vec<String> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| format!("reading {}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry
                .map_err(|e| format!("reading {}: {e}", dir.display()))?
                .path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path.to_string_lossy().into_owned());
            }
        }
    }

    let status = Command::new("rustfmt")
        .arg("--edition")
        .arg("2024")
        .args(&files)
        .status()
        .map_err(|e| format!("running rustfmt: {e}"))?;

    if !status.success() {
        return Err(format!("rustfmt failed on the generated files: {status}"));
    }

    Ok(())
}
