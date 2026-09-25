//! One function's machine code beside what each instruction ran, for
//! `instructions --asm`.
//!
//! Callgrind keeps a count per instruction when asked to (`--dump-instr`). A
//! line's count, which `callgrind_annotate` prints, folds together every
//! inlined function that line's code came from. The instructions themselves
//! say which moves, loads and calls a hot line is made of.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::error::{self, FileError, ProcessError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error("no function in the replay names '{pattern}'")]
    NoSymbol { pattern: String },
    #[error("the profile has no instruction counts; record it again")]
    NoInstructionCounts,
}

/// What one function ran, by instruction address.
#[derive(Default)]
struct Counts {
    /// Instructions run at each address.
    own: HashMap<u64, u64>,
    /// At each call site, what the call ran, and the functions it called.
    calls: HashMap<u64, (u64, Vec<String>)>,
}

/// The function named by `pattern` in `binary`, each instruction beside its
/// count per step, with the source line it came from wherever that changes.
/// Instructions that never ran are folded into a count of them.
pub fn listing(profile: &Path, binary: &Path, pattern: &str, steps: u64) -> Result<String, Error> {
    let (start, size, name) = symbol(binary, pattern)?;
    let counts = counts(profile, &name)?;
    let dumped = error::spawned(
        Command::new("objdump")
            .args(["-d", "-l", "-C", "--no-show-raw-insn", "--no-addresses"])
            .arg(format!("--start-address={start:#x}"))
            .arg(format!("--stop-address={:#x}", start + size))
            .arg(binary),
    )?;
    // `--no-addresses` leaves the address off, so each line is read back
    // against a second dump that keeps it.
    let addressed = error::spawned(
        Command::new("objdump")
            .args(["-d", "-C", "--no-show-raw-insn"])
            .arg(format!("--start-address={start:#x}"))
            .arg(format!("--stop-address={:#x}", start + size))
            .arg(binary),
    )?;
    let addresses: Vec<u64> = String::from_utf8_lossy(&addressed.stdout)
        .lines()
        .filter_map(|line| {
            let (at, rest) = line.trim_start().split_once(':')?;
            (!rest.trim().is_empty()).then(|| u64::from_str_radix(at, 16).ok())?
        })
        .collect();

    let per_step = |count: u64| count as f64 / steps as f64;
    let total: u64 = counts.own.values().sum();
    let mut report = format!("{name}: {:.1} a step of its own\n", per_step(total));
    let mut next = addresses.iter();
    let mut cold = 0;
    let mut source = String::new();
    for line in String::from_utf8_lossy(&dumped.stdout).lines() {
        let trimmed = line.trim();
        // A source location, as `-l` prints it before the code it covers.
        if trimmed.starts_with('/') || trimmed.starts_with("crates/") {
            source = shorten(trimmed);
            continue;
        }
        if !line.starts_with(|c: char| c.is_whitespace()) || trimmed.is_empty() {
            continue;
        }
        let Some(&at) = next.next() else { break };
        let own = counts.own.get(&at).copied().unwrap_or(0);
        let call = counts.calls.get(&at);
        if own == 0 && call.is_none() {
            cold += 1;
            continue;
        }
        if cold > 0 {
            report.push_str(&format!("{:>9}  ({cold} not run)\n", ""));
            cold = 0;
        }
        if !source.is_empty() {
            report.push_str(&format!("{:>9}  -- {source}\n", ""));
            source.clear();
        }
        report.push_str(&format!("{:>9.2}  {trimmed}\n", per_step(own)));
        if let Some((ran, callees)) = call {
            report.push_str(&format!(
                "{:>9}  => {:.1} a step in {}\n",
                "",
                per_step(*ran),
                callees.join(", ")
            ));
        }
    }
    Ok(report)
}

/// A source path from the repository or the toolchain down to what names it.
fn shorten(path: &str) -> String {
    for marker in ["/library/", "/src/index.crates.io-", "/crates/"] {
        if let Some((_, rest)) = path.split_once(marker) {
            return rest.to_owned();
        }
    }
    path.to_owned()
}

/// The start, size and full name of the first symbol whose name holds
/// `pattern`.
fn symbol(binary: &Path, pattern: &str) -> Result<(u64, u64, String), Error> {
    let output = error::spawned(
        Command::new("nm")
            .args(["-C", "-S", "--defined-only"])
            .arg(binary),
    )?;
    let mut found: Vec<(u64, u64, String)> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(4, ' ');
            let start = u64::from_str_radix(fields.next()?, 16).ok()?;
            let size = u64::from_str_radix(fields.next()?, 16).ok()?;
            let kind = fields.next()?;
            let name = fields.next()?;
            (matches!(kind, "t" | "T") && name.contains(pattern))
                .then(|| (start, size, name.to_owned()))
        })
        .collect();
    found.sort_by_key(|(_, size, _)| std::cmp::Reverse(*size));
    found.into_iter().next().ok_or_else(|| Error::NoSymbol {
        pattern: pattern.to_owned(),
    })
}

/// Read one number of a callgrind position, which is absolute, relative to
/// the last (`+n`, `-n`), or the last again (`*`).
fn position(field: &str, last: u64) -> Option<u64> {
    let number = |text: &str| match text.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    };
    match field.as_bytes().first()? {
        b'*' => Some(last),
        b'+' => Some(last + number(&field[1..])?),
        b'-' => Some(last - number(&field[1..])?),
        _ => number(field),
    }
}

/// What the function whose demangled name is `name` ran, by instruction.
///
/// Callgrind's file names each function once and refers to it by number
/// after that, and gives each position against the last one.
fn counts(profile: &Path, name: &str) -> Result<Counts, Error> {
    let text = error::read(profile)?;
    if !text
        .lines()
        .any(|line| line.starts_with("positions:") && line.contains("instr"))
    {
        return Err(Error::NoInstructionCounts);
    }
    let mut names: HashMap<String, String> = HashMap::new();
    let mut named = |field: &str| -> String {
        // `(id) name` defines an id, and `(id)` alone uses one.
        match field
            .strip_prefix('(')
            .and_then(|rest| rest.split_once(')'))
        {
            Some((id, rest)) if !rest.trim().is_empty() => {
                names.insert(id.to_owned(), rest.trim().to_owned());
                rest.trim().to_owned()
            }
            Some((id, _)) => names.get(id).cloned().unwrap_or_default(),
            None => field.to_owned(),
        }
    };
    let mut counts = Counts::default();
    let (mut inside, mut callee, mut in_call) = (false, String::new(), false);
    let (mut last_at, mut last_line) = (0u64, 0u64);
    for line in text.lines() {
        if let Some(field) = line.strip_prefix("fn=") {
            // A recursive function's deeper calls are named `name'2`, and are
            // the same code.
            let field = named(field);
            inside = field == name
                || field
                    .strip_prefix(name)
                    .and_then(|rest| rest.strip_prefix('\''))
                    .is_some_and(|depth| depth.chars().all(|c| c.is_ascii_digit()));
            continue;
        }
        if let Some(field) = line.strip_prefix("cfn=") {
            callee = named(field);
            continue;
        }
        if line.starts_with("calls=") {
            in_call = true;
            continue;
        }
        let first = line.as_bytes().first().copied().unwrap_or(b' ');
        if !(first.is_ascii_digit() || matches!(first, b'+' | b'-' | b'*')) {
            continue;
        }
        let mut fields = line.split_whitespace();
        let (Some(at), Some(source)) = (fields.next(), fields.next()) else {
            continue;
        };
        let (Some(at), Some(source)) = (position(at, last_at), position(source, last_line)) else {
            continue;
        };
        (last_at, last_line) = (at, source);
        let cost: u64 = fields.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        if inside {
            match in_call {
                true => {
                    let entry = counts.calls.entry(at).or_default();
                    entry.0 += cost;
                    if !entry.1.contains(&callee) {
                        entry.1.push(callee.clone());
                    }
                }
                false => *counts.own.entry(at).or_default() += cost,
            }
        }
        in_call = false;
    }
    Ok(counts)
}
