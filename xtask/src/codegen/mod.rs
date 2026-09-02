//! Turns the dumps in `dumps/` into Rust, one module per kind of content.
//! Each domain is a submodule here and a file under `generated/`. A domain reads exactly one dump and writes exactly one tree, so adding relics means adding a dump, a submodule. A line in [`DOMAINS`], nothing existing has to move.
//! Every dump carries the game's content hash in its header. [`run`] refuses to generate when two of them disagree: that means the tables in one crate came from two different builds. This is exactly the kind of drift this whole arrangement exists to make impossible.
//! The generated tree is committed rather than produced by a `build.rs`, so a content change reads as a diff. In a project about noticing when the game and the simulator disagree, "which forty cards did that patch touch" is worth being able to see.

mod acts;
mod afflictions;
mod ascension;
mod cards;
mod characters;
mod enchantments;
mod encounters;
mod events;
mod modifiers;
mod monsters;
mod orbs;
mod potions;
mod powers;
mod relics;
mod sets;
mod split;

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const VERSION_DIR: &str = "crates/sts2-content/src/v0_107_1";

/// The domains generated so far. Relics, potions and encounters each get a dump, a submodule, and an entry here.
const DOMAINS: [&str; 14] = [
    "acts",
    "afflictions",
    "ascension",
    "cards",
    "characters",
    "enchantments",
    "encounters",
    "events",
    "modifiers",
    "monsters",
    "orbs",
    "potions",
    "powers",
    "relics",
];

const DERIVES: &str = "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]";

/// Rust identifiers the game's names collide with.
///
/// `Self` is the only one that bites in practice, and it cannot be escaped as `r#Self` either, the language reserves it outright. The serde rename keeps the game's spelling on the wire, so this is a Rust-side name only.
fn sanitize(name: &str) -> &str {
    match name {
        "Self" => "SelfTarget",
        other => other,
    }
}

/// Ids are `SCREAMING_SNAKE`. Variants are `CamelCase`.
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
/// Layout only: serde goes through the variant rename, so nothing on the wire or in a dump depends on the number a variant compiles to. It is still worth choosing rather than defaulting to the wide one. This is because a discriminant sits next to other fields, an encounter's `Spawn` is a `Monster` beside a slot index. This is two bytes when `Monster` is a `u8` and four when it is a `u16`, and it appears four times in every roster.
///
/// So the domains differ: 88 cards in the largest pool and 126 monsters fit a byte, and 273 powers do not.
fn repr(variants: usize) -> Result<&'static str, String> {
    match variants {
        n if n <= u8::MAX as usize + 1 => Ok("u8"),
        n if n <= u16::MAX as usize + 1 => Ok("u16"),
        n => Err(format!(
            "{n} variants is past what even a u16 discriminant holds"
        )),
    }
}

/// The build a dump came from, as its `# build` line names it.
///
/// Two hashes, because one is not enough. `content_hash` is XxHash32 over the *set of model ids*, it moves when content is added, removed or renamed. It does not move when a card's damage goes from 6 to 7 or when a hook is added, since neither changes an id. `assembly_hash` is shipped in the game's `release_info.json` and identifies the release, so it moves on any code change at all.
///
/// A dump anchored on the content hash alone is accepted by a build that had quietly rebalanced every number in it. Both are checked.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Build {
    version: String,
    commit: String,
    assembly_hash: i64,
    content_hash: u32,
}

impl std::fmt::Display for Build {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}), assembly {}, content {}",
            self.version, self.commit, self.assembly_hash, self.content_hash
        )
    }
}

/// A dump, split into whitespace-separated rows, with the build it came from.
///
/// Comment lines are dropped except the `# build` header, which every dump carries as its second line.
struct Dump {
    build: Build,
    rows: Vec<Vec<String>>,
}

impl Dump {
    fn read(name: &str) -> Result<Dump, String> {
        let path = Path::new(VERSION_DIR).join("dumps").join(name);
        let text =
            fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;

        let build = text
            .lines()
            .find_map(|l| l.strip_prefix("# build "))
            .ok_or_else(|| format!("{name} has no `# build` header"))?;

        // Keyed rather than positional, so a field added later does not
        // silently shift the meaning of the ones already there.
        let field = |key: &str| -> Result<String, String> {
            let mut words = build.split_whitespace();
            match key {
                // The version is the first word, before any key.
                "" => words
                    .next()
                    .map(String::from)
                    .ok_or_else(|| format!("{name}'s build header is empty")),
                key => words
                    .position(|w| w == key)
                    .and_then(|at| build.split_whitespace().nth(at + 1))
                    .map(String::from)
                    .ok_or_else(|| format!("{name}'s build header names no {key}: {build}")),
            }
        };

        let number = |key: &str, raw: String| -> Result<i64, String> {
            raw.parse()
                .map_err(|e| format!("{name}'s {key} is not a number: {raw}: {e}"))
        };

        let assembly_hash = number("assembly hash", field("assembly")?)?;
        let content_hash = number("content hash", field("content")?)?;
        let content_hash = u32::try_from(content_hash)
            .map_err(|e| format!("{name}'s content hash does not fit a u32: {e}"))?;

        let rows = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| l.split_whitespace().map(String::from).collect())
            .collect();

        Ok(Dump {
            build: Build {
                version: field("")?,
                commit: field("commit")?,
                assembly_hash,
                content_hash,
            },
            rows,
        })
    }

    /// Every row of one kind, in dump order, which is load-bearing for rows whose position is their index.
    fn of(&self, kind: &str) -> impl Iterator<Item = &Vec<String>> {
        self.rows.iter().filter(move |r| r[0] == kind)
    }
}

pub fn run() -> Result<String, String> {
    let root = Path::new(VERSION_DIR).join("generated");
    fs::create_dir_all(&root).map_err(|e| format!("creating {}: {e}", root.display()))?;

    let builds = [
        acts::generate(&root)?,
        afflictions::generate(&root)?,
        ascension::generate(&root)?,
        cards::generate(&root)?,
        characters::generate(&root)?,
        enchantments::generate(&root)?,
        encounters::generate(&root)?,
        events::generate(&root)?,
        modifiers::generate(&root)?,
        monsters::generate(&root)?,
        orbs::generate(&root)?,
        potions::generate(&root)?,
        powers::generate(&root)?,
        relics::generate(&root)?,
    ];

    // Two dumps from two builds will produce a crate whose tables describe
    // neither. Cheap to check, and impossible to notice by eye. Both hashes are
    // compared, not just the content one: a rebalance that moved no ids will
    // leave the content hash equal and the numbers different.
    let build = &builds[0];
    for (domain, b) in DOMAINS.iter().zip(&builds) {
        if b != build {
            return Err(format!(
                "{domain}'s dump came from {b}, but {}'s came from {build} — \
                 they are different builds",
                DOMAINS[0]
            ));
        }
    }

    // The roll-up. One line per domain, so adding relics is adding a dump, a
    // module, and an entry in `DOMAINS`.
    let mut out = format!(
        "//! Generated by `cargo xtask codegen`. DO NOT EDIT.\n//!\n//! One module per kind of content.\n//! Source: `dumps/`, build {build}.\n\n"
    );
    for domain in DOMAINS {
        writeln!(out, "pub mod {domain};").unwrap();
    }
    let path = root.join("mod.rs");
    fs::write(&path, &out).map_err(|e| format!("writing {}: {e}", path.display()))?;

    // Formatted rather than emitted pre-formatted. `just lint` runs
    // `cargo fmt --all --check` over the whole workspace, generated files
    // included, and matching rustfmt by hand in a `writeln!` is a losing game,     // it reorders `mod` declarations and rewraps anything past the margin.
    format(&root)?;

    Ok(format!(
        "generated {VERSION_DIR}/generated/ — {} domains: {}\n  build {build}",
        DOMAINS.len(),
        DOMAINS.join(", ")
    ))
}

/// The banner every generated file opens with.
fn banner(what: &str, source: &str, build: &Build) -> String {
    format!(
        "//! Generated by `cargo xtask codegen`. DO NOT EDIT.\n//!\n//! {what}\n//! Source: `dumps/{source}`, build {build}.\n"
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
