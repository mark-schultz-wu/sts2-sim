//! Turns the dumps in `dumps/` into Rust, one module per kind of content.
//!
//! Each domain is a submodule here and a file under `generated/`. A domain
//! reads exactly one dump and writes exactly one tree. Adding relics means
//! adding a dump, a submodule, and a line in [`DOMAINS`]. Nothing existing has
//! to move.
//!
//! Every dump carries the game's content hash in its header, and [`run`] refuses
//! to generate when two of them disagree. That means the tables in one crate
//! came from two different builds. It is exactly the kind of drift this whole
//! arrangement exists to make impossible.
//!
//! The generated tree is committed rather than produced by a `build.rs`, so a
//! content change reads as a diff. In a project about noticing when the game
//! and the simulator disagree, "which forty cards did that patch touch" is
//! worth being able to see.

mod acts;
mod afflictions;
mod ascension;
mod bridge;
mod card_plays;
mod cards;
mod characters;
mod enchantments;
mod encounters;
mod events;
mod hoist;
mod ids;
mod mad_science;
mod modifiers;
mod monster_moves;
mod monsters;
mod orbs;
mod potions;
mod powers;
mod relics;
mod restsite;
mod runstate;
mod sets;
mod split;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{self, FileError, ProcessError};

/// What reading a dump or writing a table found wrong, whichever domain it
/// was for.
#[derive(Debug, thiserror::Error)]
pub enum DumpError {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Version(#[from] VersionError),
    #[error("{dump} has no `# build` header")]
    NoHeader { dump: String },
    #[error("{dump}'s build header is empty")]
    EmptyHeader { dump: String },
    #[error("{dump}'s build header names no {key}")]
    NoHeaderKey { dump: String, key: &'static str },
    #[error("{dump}'s {key} is not a number: {text}: {source}")]
    HeaderNumber {
        dump: String,
        key: &'static str,
        text: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("{dump}'s content hash {hash} does not fit a u32")]
    ContentHashRange { dump: String, hash: i64 },
    #[error("{dump}'s build header names no content hash")]
    NoContentHash { dump: String },
    #[error("{dump} was dumped from another build than the table it feeds")]
    OtherBuild { dump: &'static str },
    #[error("{dump} was read from {read}, but the tables come from {tables}")]
    ReadFrom {
        dump: &'static str,
        read: Box<Build>,
        tables: Box<Build>,
    },
    #[error("{id}: {got} columns, not {want}")]
    Columns { id: String, got: usize, want: usize },
    #[error("{at}: {text} is not a number: {source}")]
    Number {
        at: String,
        text: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("the dump declares no {0}")]
    Empty(&'static str),
    #[error("{0} variants is past what even a u16 discriminant holds")]
    TooManyVariants(usize),
    #[error(transparent)]
    Rustfmt(#[from] ProcessError),
}

impl DumpError {
    /// A cell read as a number, or the failure that names where it is.
    fn number<T: std::str::FromStr<Err = std::num::ParseIntError>>(
        at: impl std::fmt::Display,
        text: &str,
    ) -> Result<T, DumpError> {
        text.parse().map_err(|source| DumpError::Number {
            at: at.to_string(),
            text: text.to_owned(),
            source,
        })
    }
}

/// A build named that has no lock.
#[derive(Debug, thiserror::Error)]
#[error("unknown game version '{name}': no {}", lock.display())]
pub struct VersionError {
    name: String,
    lock: PathBuf,
}

/// Every domain's failure, by the domain.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Dump(#[from] DumpError),
    #[error("acts: {0}")]
    Acts(#[from] acts::Error),
    #[error("afflictions: {0}")]
    Afflictions(#[from] afflictions::Error),
    #[error("ascension: {0}")]
    Ascension(#[from] ascension::Error),
    #[error("card plays: {0}")]
    CardPlays(#[from] card_plays::Error),
    #[error("cards: {0}")]
    Cards(#[from] cards::Error),
    #[error("characters: {0}")]
    Characters(#[from] characters::Error),
    #[error("enchantments: {0}")]
    Enchantments(#[from] enchantments::Error),
    #[error("encounters: {0}")]
    Encounters(#[from] encounters::Error),
    #[error("events: {0}")]
    Events(#[from] events::Error),
    #[error("modifiers: {0}")]
    Modifiers(#[from] modifiers::Error),
    #[error("monster moves: {0}")]
    MonsterMoves(#[from] monster_moves::Error),
    #[error("monsters: {0}")]
    Monsters(#[from] monsters::Error),
    #[error("orbs: {0}")]
    Orbs(#[from] orbs::Error),
    #[error("potions: {0}")]
    Potions(#[from] potions::Error),
    #[error("powers: {0}")]
    Powers(#[from] powers::Error),
    #[error("relics: {0}")]
    Relics(#[from] relics::Error),
    #[error("rest site: {0}")]
    Restsite(#[from] restsite::Error),
    #[error("run state: {0}")]
    Runstate(#[from] runstate::Error),
    #[error(
        "{domain}'s dump came from {theirs}, but {first}'s came from {ours}: they are different builds"
    )]
    Builds {
        domain: &'static str,
        theirs: Box<Build>,
        first: &'static str,
        ours: Box<Build>,
    },
}

/// The build to read dumps from and generate tables for.
///
/// `.envrc` exports `STS2_GAME_VERSION` and `mod/build.sh` reads the same name,
/// so one setting points the mods, the dumps and the tables at one build.
///
/// A name with no lock under `mod/games/` is a typo. Catching it here stops a
/// dump landing in a directory nobody meant to create.
pub(crate) fn version() -> Result<String, VersionError> {
    let name = std::env::var("STS2_GAME_VERSION").unwrap_or_else(|_| "v0.107.1".to_string());
    let lock = Path::new("mod/games").join(format!("{name}.lock"));
    if !lock.is_file() {
        return Err(VersionError { name, lock });
    }
    Ok(name)
}

/// The build as a Rust module name. `v0.107.1` is `v0_107_1`.
pub(crate) fn version_module() -> Result<String, VersionError> {
    Ok(version()?.replace('.', "_"))
}

/// Where one build's dumps and generated tables live.
pub(crate) fn version_dir() -> Result<PathBuf, VersionError> {
    Ok(Path::new("crates/sts2-content/src").join(version_module()?))
}

/// The domains generated so far. Relics, potions and encounters each get a
/// dump, a submodule, and an entry here.
const DOMAINS: [&str; 16] = [
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
    "restsite",
    "runstate",
];

const DERIVES: &str = "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]";

/// Rust identifiers the game's names collide with.
///
/// `Self` is the only one that bites in practice, and it cannot be escaped as
/// `r#Self` either, because the language reserves it outright. The serde rename
/// keeps the game's spelling on the wire, so this is a Rust-side name only.
fn sanitize(name: &str) -> &str {
    match name {
        "Self" => "SelfTarget",
        other => other,
    }
}

/// Ids are `SCREAMING_SNAKE`, and variants are `CamelCase`.
pub(crate) fn camel(id: &str) -> String {
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

/// Every row of a table, as one array `data` indexes.
///
/// The alternative is a match with an arm per value, which copies a whole row
/// to read one field of it.
///
/// No variant carries an explicit discriminant. So a value numbers itself by
/// where it is declared, and that is the order the rows are written in.
fn rows(out: &mut String, data_ty: &str, rows: &[String]) {
    shared_rows(out, "", data_ty, rows);
}

/// [`rows`], with the table itself seen as far as `vis` says. An outer enum
/// over several tables reads each one directly.
fn shared_rows(out: &mut String, vis: &str, data_ty: &str, rows: &[String]) {
    writeln!(
        out,
        "/// Every row, in the order the enum declares its variants, which is\n\
         /// the index a value casts to."
    )
    .unwrap();
    writeln!(out, "{vis}static ROWS: [{data_ty}; {}] = [", rows.len()).unwrap();
    for row in rows {
        writeln!(out, "    {row},").unwrap();
    }
    writeln!(out, "];\n").unwrap();
}

/// One list per value, as one array the accessor indexes.
///
/// The same shape as [`rows`], for an accessor that answers with a slice
/// rather than a row.
fn lists(out: &mut String, name: &str, elem_ty: &str, lists: &[String]) {
    writeln!(
        out,
        "/// One entry per value, in the order the enum declares its variants."
    )
    .unwrap();
    writeln!(out, "static {name}: [&[{elem_ty}]; {}] = [", lists.len()).unwrap();
    for list in lists {
        writeln!(out, "    &[{list}],").unwrap();
    }
    writeln!(out, "];\n").unwrap();
}

/// The narrowest discriminant that holds this many variants.
///
/// Layout only: serde goes through the variant rename, so nothing on the wire
/// or in a dump depends on the number a variant compiles to. The xtask README
/// says why it is still worth choosing.
fn repr(variants: usize) -> Result<&'static str, DumpError> {
    match variants {
        n if n <= u8::MAX as usize + 1 => Ok("u8"),
        n if n <= u16::MAX as usize + 1 => Ok("u16"),
        n => Err(DumpError::TooManyVariants(n)),
    }
}

/// The build a dump came from, as its `# build` line names it.
///
/// Two hashes, because one is not enough. Both are checked, and the xtask
/// README says what each one moves on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Build {
    version: String,
    commit: String,
    assembly_hash: i64,
    /// `None` for a dump read off the assembly, which has no running game to
    /// work the hash out.
    content_hash: Option<u32>,
}

impl std::fmt::Display for Build {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}), assembly {}",
            self.version, self.commit, self.assembly_hash
        )?;
        match self.content_hash {
            Some(hash) => write!(f, ", content {hash}"),
            None => Ok(()),
        }
    }
}

/// A dump, split into whitespace-separated rows, with the build it came from.
///
/// Comment lines are dropped except the `# build` header, which every dump
/// carries as its second line.
struct Dump {
    build: Build,
    rows: Vec<Vec<String>>,
}

impl Dump {
    /// A dump the game printed, whose header names the content hash.
    fn read(name: &str) -> Result<Dump, DumpError> {
        let dump = Dump::read_any(name)?;
        match dump.build.content_hash {
            Some(_) => Ok(dump),
            None => Err(DumpError::NoContentHash {
                dump: name.to_owned(),
            }),
        }
    }

    /// A dump whose header need not name the content hash, which is one read
    /// off the decompiled assembly.
    fn read_any(name: &str) -> Result<Dump, DumpError> {
        let path = version_dir()?.join("dumps").join(name);
        let text = error::read(&path)?;
        let dump = || name.to_owned();

        let build = text
            .lines()
            .find_map(|l| l.strip_prefix("# build "))
            .ok_or_else(|| DumpError::NoHeader { dump: dump() })?;

        // Keyed rather than positional, so a field added later does not
        // silently shift the meaning of the ones already there.
        let field = |key: &'static str| -> Result<String, DumpError> {
            let mut words = build.split_whitespace();
            match key {
                // The version is the first word, before any key.
                "" => words
                    .next()
                    .map(String::from)
                    .ok_or_else(|| DumpError::EmptyHeader { dump: dump() }),
                key => words
                    .position(|w| w == key)
                    .and_then(|at| build.split_whitespace().nth(at + 1))
                    .map(String::from)
                    .ok_or_else(|| DumpError::NoHeaderKey { dump: dump(), key }),
            }
        };

        let number = |key: &'static str, text: String| -> Result<i64, DumpError> {
            text.parse().map_err(|source| DumpError::HeaderNumber {
                dump: dump(),
                key,
                text,
                source,
            })
        };

        let assembly_hash = number("assembly hash", field("assembly")?)?;
        let content_hash = match field("content") {
            Ok(raw) => {
                let hash = number("content hash", raw)?;
                Some(
                    u32::try_from(hash)
                        .map_err(|_| DumpError::ContentHashRange { dump: dump(), hash })?,
                )
            }
            Err(_) => None,
        };

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

    /// Every row of one kind, in dump order, which is load-bearing for rows
    /// whose position is their index.
    fn of(&self, kind: &str) -> impl Iterator<Item = &Vec<String>> {
        self.rows.iter().filter(move |r| r[0] == kind)
    }
}

/// The hooks each model's class overrides, by the model's id, from the hook
/// dump. Only the models of `category`, the id's own, since two kinds can
/// share an entry. Only the hooks some [`sts2_core::EventHook`] is raised as.
fn overrides(
    build: &Build,
    category: &str,
) -> Result<BTreeMap<String, Vec<sts2_core::ModelHook>>, DumpError> {
    let hooks = Dump::read("hooks.txt")?;
    if hooks.build != *build {
        return Err(DumpError::OtherBuild { dump: "hooks.txt" });
    }
    let mut overrides: BTreeMap<String, Vec<sts2_core::ModelHook>> = BTreeMap::new();
    for row in hooks.of("implements").filter(|row| row[4] == category) {
        if let Some(hook) = sts2_core::ModelHook::ALL
            .into_iter()
            .find(|hook| hook.name() == row[2])
        {
            overrides.entry(row[1].clone()).or_default().push(hook);
        }
    }
    Ok(overrides)
}

/// `static OVERRIDES`, one entry per id in the order given.
fn emit_overrides<'a>(
    out: &mut String,
    doc: &str,
    ids: impl ExactSizeIterator<Item = &'a str>,
    overrides: &BTreeMap<String, Vec<sts2_core::ModelHook>>,
) {
    writeln!(
        out,
        "\n/// The hooks each class overrides, from `hooks.txt`, {doc}\n\
         static OVERRIDES: [sts2_core::Overrides; {}] = [",
        ids.len()
    )
    .unwrap();
    for id in ids {
        let mut entry = String::from("sts2_core::Overrides::NONE");
        for hook in overrides.get(id).into_iter().flatten() {
            write!(entry, ".with(sts2_core::ModelHook::{hook:?})").unwrap();
        }
        writeln!(out, "    // {id}\n    {entry},").unwrap();
    }
    writeln!(out, "];").unwrap();
}

/// `cargo xtask codegen`'s arguments.
#[derive(clap::Args)]
pub struct Args {
    /// Generate only the build `STS2_GAME_VERSION` names, and leave what the
    /// builds share alone. Each build's run of the whole is one of these.
    #[arg(long, hide = true)]
    one: bool,
}

/// Every build's tables, then what they share.
///
/// A type is shared only when every build generated it alike. So every build
/// is generated afresh first, and none still holds the last run's re-exports.
pub fn run(args: Args) -> Result<String, Error> {
    if args.one {
        return run_one();
    }
    let mut out = String::new();
    let mut locks: Vec<PathBuf> = error::entries(Path::new("mod/games"))
        .map_err(DumpError::from)?
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "lock"))
        .collect();
    locks.sort();
    let exe = std::env::current_exe().map_err(|source| {
        DumpError::from(FileError {
            verb: error::Verb::Reading,
            path: PathBuf::from("current_exe"),
            source,
        })
    })?;
    for lock in &locks {
        let name = lock.file_stem().unwrap_or_default();
        let made = error::output(
            Command::new(&exe)
                .args(["codegen", "--one"])
                .env("STS2_GAME_VERSION", name),
        )
        .map_err(DumpError::from)?;
        out.push_str(&String::from_utf8_lossy(&made.stdout));
    }

    let src = Path::new("crates/sts2-content/src");
    let builds = ids::builds(src)?;
    let mut written = hoist::hoist(src, &builds)?;
    written.push(ids::generate(src)?);
    error::status(
        Command::new("rustfmt")
            .arg("--edition")
            .arg("2024")
            .args(&written),
    )
    .map_err(DumpError::from)?;
    writeln!(out, "hoisted into {}/generated/", src.display()).unwrap();
    Ok(out)
}

/// One build's tables.
fn run_one() -> Result<String, Error> {
    let dir = version_dir().map_err(DumpError::from)?;
    let root = dir.join("generated");
    error::create_dir(&root).map_err(DumpError::from)?;

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
        restsite::generate(&root)?,
        runstate::generate(&root)?,
    ];

    // Two dumps from two builds would produce a crate whose tables describe
    // neither. Cheap to check, and impossible to notice by eye. Both hashes are
    // compared, not just the content one: a rebalance that moved no ids would
    // leave the content hash equal and the numbers different.
    let build = &builds[0];
    for (domain, b) in DOMAINS.iter().zip(&builds) {
        if b != build {
            return Err(Error::Builds {
                domain,
                theirs: Box::new(b.clone()),
                first: DOMAINS[0],
                ours: Box::new(build.clone()),
            });
        }
    }

    // Read off the assembly rather than printed by the game, so checked
    // against the tables' build by hand.
    card_plays::generate(&root, build)?;
    monster_moves::generate(&root, build)?;

    // The roll-up. One line per domain, so adding relics is adding a dump, a
    // module, and an entry in `DOMAINS`.
    let mut out = format!(
        "//! Generated by `cargo xtask codegen`. DO NOT EDIT.\n//!\n//! One module per kind of content.\n//! Source: `dumps/`, build {build}.\n\n"
    );
    for domain in DOMAINS {
        writeln!(out, "pub mod {domain};").unwrap();
    }
    writeln!(out, "pub mod card_plays;").unwrap();
    writeln!(out, "pub mod monster_moves;").unwrap();
    let path = root.join("mod.rs");
    error::write(&path, &out).map_err(DumpError::from)?;

    // Formatted rather than emitted pre-formatted. `just lint` runs
    // `cargo fmt --all --check` over the whole workspace, generated files
    // included, and matching rustfmt by hand in a `writeln!` is a losing game —
    // it reorders `mod` declarations and rewraps anything past the margin.
    format(&root)?;

    Ok(format!(
        "generated {}/generated/ — {} domains: {}\n  build {build}",
        dir.display(),
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
fn format(root: &Path) -> Result<(), DumpError> {
    let mut files: Vec<String> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        for path in error::entries(&dir)? {
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path.to_string_lossy().into_owned());
            }
        }
    }

    error::status(
        Command::new("rustfmt")
            .arg("--edition")
            .arg("2024")
            .args(&files),
    )?;
    Ok(())
}
