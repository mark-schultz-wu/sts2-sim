//! Which of the rules' code the runs the game played reach, in code regions.
//!
//! The recorded runs and the scenarios replay under `cargo llvm-cov`, which
//! counts each region of code they run. A run replays only as far as it
//! agrees with the game, so a reached region ran while the two agreed. A
//! generic rule compiles once per build, and either build reaching it counts.
//!
//! A region whose code is only a panic, such as `unimplemented!()`, is counted
//! apart from the rest. Each other unreached region of the content's rules is
//! named by the content above it, such as `V::Card::BASH`, or by its file.
//!
//! ```text
//! cargo xtask reached
//! cargo xtask reached --missing
//! cargo xtask reached --from target/llvm-cov/reached.json
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{self, FileError, ProcessError};

/// The tests that replay what the game played.
const TESTS: [&str; 2] = ["recorded", "scenarios"];

/// The macros a region can be nothing but, which no run means to reach.
const PANICS: [&str; 4] = ["unimplemented!", "unreachable!", "panic!", "todo!"];

/// The content a match arm names, as `V::<kind>::<NAME>`.
const KINDS: [&str; 11] = [
    "Card",
    "Relic",
    "Power",
    "Potion",
    "Monster",
    "Event",
    "Enchantment",
    "Orb",
    "Modifier",
    "Encounter",
    "Affliction",
];

#[derive(clap::Args)]
pub struct Args {
    /// Name each unreached region, under its content.
    #[arg(long)]
    missing: bool,
    /// Read this export of an earlier run rather than replay again.
    #[arg(long)]
    from: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error("{0}: not an llvm-cov export: {1}")]
    Export(PathBuf, String),
}

/// One region of code: its file, where it starts and ends, as line and
/// column.
type Span = (String, [u64; 4]);

/// The lines one content's unreached regions start on, by file and line
/// number, with the line's code.
type Lines = BTreeMap<(String, u64), String>;

/// One area's regions: reached, only a panic and not reached, and every
/// other one not reached.
#[derive(Default)]
struct Counts {
    reached: usize,
    panics: usize,
    unreached: usize,
}

pub fn run(Args { missing, from }: Args) -> Result<String, Error> {
    let root = crate::workspace();
    let export = match from {
        Some(export) => export,
        None => replay(&root)?,
    };
    let text = error::read(&export)?;
    let spans = spans(&text).map_err(|e| Error::Export(export.clone(), e))?;
    let mut sources = Sources::default();

    let mut areas: BTreeMap<String, Counts> = BTreeMap::new();
    // Pool, then content, then each line its unreached regions start on. A
    // line often holds several regions, and is named once.
    let mut unreached: BTreeMap<String, BTreeMap<String, Lines>> = BTreeMap::new();
    for ((file, at), count) in &spans {
        let Some(area) = area(file) else {
            continue;
        };
        let counts = areas.entry(area.clone()).or_default();
        if *count > 0 {
            counts.reached += 1;
            continue;
        }
        let code = sources.text(file, *at);
        let trimmed = code.trim_start_matches(|c: char| c == '{' || c.is_whitespace());
        if PANICS.iter().any(|panic| trimmed.starts_with(panic)) {
            counts.panics += 1;
            continue;
        }
        counts.unreached += 1;
        if let Some(pool) = area.strip_prefix("content ") {
            let item = sources.item(file, at[0]);
            let line = sources
                .lines(file)
                .get(at[0] as usize - 1)
                .map_or_else(String::new, |line| line.trim().to_owned());
            unreached
                .entry(pool.to_owned())
                .or_default()
                .entry(item)
                .or_default()
                .entry((relative(&root, file), at[0]))
                .or_insert(line);
        }
    }

    let mut out = String::new();
    writeln!(
        out,
        "code regions the recorded runs and scenarios reach, by area"
    )
    .unwrap();
    writeln!(
        out,
        "  {:<36} {:>7} {:>7} {:>6} {:>11}",
        "", "reached", "of", "", "only panic"
    )
    .unwrap();
    for (area, counts) in &areas {
        let of = counts.reached + counts.unreached;
        writeln!(
            out,
            "  {area:<36} {:>7} {of:>7} {:>5.1}% {:>11}",
            counts.reached,
            100.0 * counts.reached as f64 / of.max(1) as f64,
            counts.panics,
        )
        .unwrap();
    }
    writeln!(
        out,
        "\ncontent with code no run reaches, by pool, in lines, most first"
    )
    .unwrap();
    for (pool, items) in &unreached {
        let mut items: Vec<_> = items.iter().collect();
        items.sort_by_key(|(item, lines)| (std::cmp::Reverse(lines.len()), *item));
        let lines: usize = items.iter().map(|(_, lines)| lines.len()).sum();
        let named: Vec<String> = items
            .iter()
            .take(match missing {
                true => items.len(),
                false => 12,
            })
            .map(|(item, lines)| format!("{item} {}", lines.len()))
            .collect();
        writeln!(
            out,
            "  {pool}: {} items, {lines} lines: {}",
            items.len(),
            named.join(", ")
        )
        .unwrap();
        if missing {
            for (item, lines) in &items {
                writeln!(out, "    {item}").unwrap();
                for ((file, at), line) in *lines {
                    writeln!(out, "      {file}:{at} {line}").unwrap();
                }
            }
        }
    }
    Ok(out)
}

/// Replays the tests under `cargo llvm-cov`, and exports what they ran.
fn replay(root: &Path) -> Result<PathBuf, Error> {
    error::status(Command::new("cargo").current_dir(root).args([
        "llvm-cov",
        "clean",
        "--profraw-only",
    ]))?;
    let mut tests = Command::new("cargo");
    tests
        .current_dir(root)
        .args(["llvm-cov", "--no-report", "-p", "sts2-golden"]);
    for test in TESTS {
        tests.args(["--test", test]);
    }
    error::status(&mut tests)?;
    let export = root.join("target/llvm-cov/reached.json");
    error::create_dir(export.parent().expect("the export sits in a directory"))?;
    error::status(Command::new("cargo").current_dir(root).args([
        "llvm-cov",
        "report",
        "--json",
        "--output-path",
        export.to_str().expect("the workspace's path is text"),
    ]))?;
    Ok(export)
}

/// Every code region of the export, with the most times any build's copy of
/// it ran.
fn spans(text: &str) -> Result<BTreeMap<Span, u64>, String> {
    let export: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let functions = export["data"][0]["functions"]
        .as_array()
        .ok_or("no functions")?;
    let mut spans = BTreeMap::new();
    for function in functions {
        let files = function["filenames"].as_array().ok_or("no file names")?;
        for region in function["regions"].as_array().ok_or("no regions")? {
            let numbers: Vec<u64> = region
                .as_array()
                .ok_or("a region that is not a list")?
                .iter()
                .map(|n| n.as_u64().unwrap_or(0))
                .collect();
            // Line, column, line, column, count, file, expanded file, kind.
            let [l0, c0, l1, c1, count, file, _, kind, ..] = numbers[..] else {
                return Err("a region with fewer than eight numbers".into());
            };
            // Kind 0 is code. The rest are expansions, skipped and gap
            // regions.
            if kind != 0 {
                continue;
            }
            let file = files[file as usize].as_str().ok_or("a file name")?;
            let seen = spans
                .entry((file.to_owned(), [l0, c0, l1, c1]))
                .or_insert(0);
            *seen = (*seen).max(count);
        }
    }
    Ok(spans)
}

/// The area a file's regions count under: the content's rules by pool, or the
/// simulator by module. Nothing for anything else.
fn area(file: &str) -> Option<String> {
    let first = |rest: &str| {
        rest.split('/')
            .next()
            .unwrap_or(rest)
            .trim_end_matches(".rs")
            .to_owned()
    };
    if let Some((_, rest)) = file.split_once("/sts2-content/src/behaviour/") {
        return Some(format!("content {}", first(rest)));
    }
    if let Some((_, rest)) = file.split_once("/sts2-content/src/")
        && let Some((build, rest)) = rest.split_once("/behaviour/")
    {
        return Some(format!("content {build} {}", first(rest)));
    }
    let (_, rest) = file.split_once("/sts2-sim/src/")?;
    Some(format!("sim {}", first(rest)))
}

/// `file` from the workspace's root.
fn relative(root: &Path, file: &str) -> String {
    Path::new(file)
        .strip_prefix(root)
        .map_or_else(|_| file.to_owned(), |path| path.display().to_string())
}

/// The source files, read once each.
#[derive(Default)]
struct Sources(BTreeMap<String, Vec<String>>);

impl Sources {
    fn lines(&mut self, file: &str) -> &[String] {
        self.0.entry(file.to_owned()).or_insert_with(|| {
            std::fs::read_to_string(file)
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect()
        })
    }

    /// The code `at` spans, from its first line and column to its last.
    fn text(&mut self, file: &str, at: [u64; 4]) -> String {
        let [l0, c0, l1, c1] = at.map(|n| n as usize);
        let lines = self.lines(file);
        let Some(chosen) = lines.get(l0.saturating_sub(1)..l1.min(lines.len())) else {
            return String::new();
        };
        let mut text = chosen.to_vec();
        if let Some(last) = text.last_mut() {
            last.truncate(c1.saturating_sub(1).min(last.len()));
        }
        if let Some(first) = text.first_mut() {
            *first = first.get(c0.saturating_sub(1)..).unwrap_or("").to_owned();
        }
        text.join("\n")
    }

    /// The content a region at `line` belongs to: the nearest content or
    /// resume point named at or above it inside its function, or its file.
    fn item(&mut self, file: &str, line: u64) -> String {
        let lines = self.lines(file);
        let mut at = (line as usize).min(lines.len());
        while at > 0 {
            let text = &lines[at - 1];
            if let Some(named) = named(text) {
                return named;
            }
            let trimmed = text.trim_start();
            if at < line as usize
                && (trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("pub(crate) fn ")
                    || trimmed.starts_with("pub(super) fn "))
            {
                break;
            }
            at -= 1;
        }
        let path = Path::new(file);
        match path.file_stem().and_then(|stem| stem.to_str()) {
            Some("mod") | None => path
                .parent()
                .and_then(|dir| dir.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("?")
                .to_owned(),
            Some(stem) => stem.to_owned(),
        }
    }
}

/// The content `line` names, as `V::Card::BASH` names `BASH`, or the resume
/// point it names, as `Resume::ThrashHit` names `ThrashHit`.
fn named(line: &str) -> Option<String> {
    for kind in KINDS {
        let marker = format!("V::{kind}::");
        if let Some((_, rest)) = line.split_once(&marker) {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    let (_, rest) = line.split_once("Resume::")?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    (name.chars().next()?.is_ascii_uppercase()).then_some(name)
}
