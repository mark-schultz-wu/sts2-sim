//! What one replayed step costs, in instructions, under callgrind.
//!
//! A count is exact and repeatable, so two builds compare without a timer's
//! noise. Caches and branch prediction are the criterion benches' business.
//!
//! The replay is `sts2-golden`'s `replay_sites`, through the rules alone.
//! Map generation is left out: every run makes its maps once, and a search
//! inside a fight never does.
//!
//! A profile is kept under `target/profile/<label>.callgrind`. `--vs` holds
//! it against an earlier one, and `--top` lists the costliest functions.
//! `--callers` lists what calls a function, and `--asm` lists a function's
//! code with what each instruction ran. `--map` counts map generation alone.
//!
//! ```text
//! cargo xtask instructions before
//! cargo xtask instructions after --vs before --top 10
//! cargo xtask instructions after --callers malloc.c:malloc
//! cargo xtask instructions after --asm listen::raise
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Map generation, as callgrind names it: `ActMap::generate`, which builds
/// the map, lists its nodes and drops the builder. The builder's own
/// `ActMapBuilder<` does not match `ActMap<`, and a closure inside ends in
/// `}` rather than `>`.
const MAP: &str = "*::ActMap<*>>::generate::<*>";

/// The simulator's own functions, which `--top` lists.
const OURS: &str = "sts2_";

/// One profile's figures.
struct Profile {
    total: u64,
    steps: u64,
}

impl Profile {
    fn per_step(&self) -> f64 {
        self.total as f64 / self.steps as f64
    }
}

#[derive(clap::Args)]
pub struct Args {
    /// What to call the profile.
    #[arg(default_value = "current")]
    label: String,
    /// Another profile's label, to compare this one against.
    #[arg(long)]
    vs: Option<String>,
    /// How many functions to list, costliest first.
    #[arg(long, default_value_t = 0)]
    top: usize,
    /// The functions that call these, and what each call costs.
    #[arg(long)]
    callers: Option<String>,
    /// A function to list the instructions of.
    #[arg(long)]
    asm: Option<String>,
    /// Map generation alone, rather than everything else.
    #[arg(long)]
    map: bool,
}

pub fn run(args: Args) -> Result<String, String> {
    let Args {
        label,
        vs,
        top,
        callers,
        asm,
        map,
    } = args;

    let root = workspace()?;
    let dir = root.join("target/profile");
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;

    let built = Command::new("cargo")
        .args([
            "build",
            "-q",
            "--release",
            "-p",
            "sts2-golden",
            "--example",
            "replay_sites",
        ])
        .current_dir(&root)
        .status()
        .map_err(|e| format!("running cargo: {e}"))?;
    if !built.success() {
        return Err("the replay did not build".into());
    }

    let out = profile_path(&dir, &label);
    let replayed = Command::new("valgrind")
        .arg("--tool=callgrind")
        // Counting from the start, so the toggle turns it off inside the
        // builder. Naming a toggle turns counting off at the start, so this
        // comes after it. For the map alone, counting starts off, and the
        // toggle turns it on inside the builder.
        .arg(format!("--toggle-collect={MAP}"))
        .arg(format!(
            "--collect-atstart={}",
            if map { "no" } else { "yes" }
        ))
        // A count per instruction as well as per line, which `--asm` reads.
        .arg("--dump-instr=yes")
        .arg(format!("--callgrind-out-file={}", out.display()))
        .arg(root.join("target/release/examples/replay_sites"))
        .arg("rules")
        .current_dir(&root)
        .output()
        .map_err(|e| format!("running valgrind, which this needs installed: {e}"))?;
    if !replayed.status.success() {
        return Err(format!(
            "the replay failed under callgrind:\n{}",
            String::from_utf8_lossy(&replayed.stderr)
        ));
    }
    let steps = divide_by(&String::from_utf8_lossy(&replayed.stdout))?;
    std::fs::write(steps_path(&dir, &label), steps.to_string())
        .map_err(|e| format!("keeping the step count: {e}"))?;

    let current = read(&dir, &label)?;
    let mut report = format!(
        "{label}: {steps} steps, {} {} map generation, {:.0} per step\n",
        millions(current.total),
        if map { "inside" } else { "outside" },
        current.per_step(),
    );
    if let Some(pattern) = &callers {
        report.push_str(&callers_of(&out, pattern, current.steps)?);
    }
    if let Some(pattern) = &asm {
        let binary = root.join("target/release/examples/replay_sites");
        report.push_str(&crate::asm::listing(&out, &binary, pattern, current.steps)?);
    }
    let Some(vs) = vs else {
        if top > 0 {
            report.push_str(&format!("the {top} costliest of ours, inclusive:\n"));
            for (name, cost) in ours(&out)?.into_iter().take(top) {
                report.push_str(&format!("  {:>9}  {name}\n", millions(cost)));
            }
        }
        return Ok(report);
    };
    let before = read(&dir, &vs)?;
    let change = (current.per_step() / before.per_step() - 1.0) * 100.0;
    report.push_str(&format!(
        "  against {vs}    {:.0} -> {:.0} per step ({change:+.1}%)\n",
        before.per_step(),
        current.per_step(),
    ));
    if top > 0 {
        // Per step on each side, so two replays of different lengths compare.
        let now: BTreeMap<String, f64> = ours(&out)?
            .into_iter()
            .map(|(name, cost)| (name, cost as f64 / current.steps as f64))
            .collect();
        let then: BTreeMap<String, f64> = ours(&profile_path(&dir, &vs))?
            .into_iter()
            .map(|(name, cost)| (name, cost as f64 / before.steps as f64))
            .collect();
        let mut moved: Vec<(&String, f64, f64)> = now
            .keys()
            .chain(then.keys())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|name| {
                let at = |costs: &BTreeMap<String, f64>| costs.get(name).copied().unwrap_or(0.0);
                (name, at(&then), at(&now))
            })
            .collect();
        moved.sort_by(|a, b| (b.2 - b.1).abs().total_cmp(&(a.2 - a.1).abs()));
        report.push_str(&format!(
            "the {top} of ours that moved most, inclusive, per step:\n"
        ));
        for (name, then, now) in moved.into_iter().take(top) {
            report.push_str(&format!(
                "  {then:>8.0} -> {now:>8.0} ({:+6.0})  {name}\n",
                now - then
            ));
        }
    }
    Ok(report)
}

/// The repository's root, which is where cargo and the profiles live.
pub(crate) fn workspace() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask sits inside the workspace".into())
}

fn profile_path(dir: &Path, label: &str) -> PathBuf {
    dir.join(format!("{label}.callgrind"))
}

fn steps_path(dir: &Path, label: &str) -> PathBuf {
    dir.join(format!("{label}.steps"))
}

/// The step count the replay prints, as `divide by <n>`.
pub(crate) fn divide_by(stdout: &str) -> Result<u64, String> {
    stdout
        .split("divide by ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| format!("the replay printed no step count:\n{stdout}"))
}

/// A kept profile's figures.
fn read(dir: &Path, label: &str) -> Result<Profile, String> {
    let file = profile_path(dir, label);
    if !file.exists() {
        return Err(format!(
            "no profile named '{label}' under {}",
            dir.display()
        ));
    }
    let steps = std::fs::read_to_string(steps_path(dir, label))
        .map_err(|e| format!("reading {label}'s step count: {e}"))?
        .trim()
        .parse()
        .map_err(|e| format!("{label}'s step count: {e}"))?;
    let total = annotate(&file, false)?
        .lines()
        .find(|line| line.contains("PROGRAM TOTALS"))
        .and_then(count)
        .ok_or("callgrind_annotate printed no total")?;
    Ok(Profile { total, steps })
}

/// The simulator's own functions, inclusive, costliest first.
fn ours(file: &Path) -> Result<Vec<(String, u64)>, String> {
    let annotated = annotate(file, true)?;
    let mut seen = std::collections::BTreeSet::new();
    let mut ours = Vec::new();
    for line in annotated.lines() {
        let Some(cost) = count(line) else { continue };
        let Some(name) = function(line) else { continue };
        // A name ending in `'2` is callgrind's entry for a recursive call,
        // which counts every level again.
        let recursive = name.rsplit_once('\'').is_some_and(|(_, depth)| {
            !depth.is_empty() && depth.chars().all(|c| c.is_ascii_digit())
        });
        if !name.contains(OURS) || recursive || !seen.insert(name.clone()) {
            continue;
        }
        ours.push((name, cost));
    }
    Ok(ours)
}

fn annotate(file: &Path, inclusive: bool) -> Result<String, String> {
    let output = Command::new("callgrind_annotate")
        .arg(format!(
            "--inclusive={}",
            if inclusive { "yes" } else { "no" }
        ))
        .arg("--threshold=100")
        .arg(file)
        .output()
        .map_err(|e| format!("running callgrind_annotate: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The count a `callgrind_annotate` line opens with.
fn count(line: &str) -> Option<u64> {
    line.split_whitespace()
        .next()?
        .replace(',', "")
        .parse()
        .ok()
}

/// The function a `callgrind_annotate` line names, without its file or the
/// binary it came from.
fn function(line: &str) -> Option<String> {
    let named = line.split_once(')')?.1.trim();
    let named = named.rsplit_once(" [").map_or(named, |(name, _)| name);
    let name = named.split_once(':').map_or(named, |(_, name)| name);
    // A call count, which some entries carry and others of the same function
    // do not.
    let name = match name.rsplit_once(" (") {
        Some((name, count)) if count.ends_with("x)") => name,
        _ => name,
    };
    Some(name.to_owned())
}

/// Each function whose name holds `pattern`, what it runs itself, and each
/// caller with the part of it that caller runs, per step. Callers under a
/// hundredth of the function are left out.
fn callers_of(file: &Path, pattern: &str, steps: u64) -> Result<String, String> {
    let output = Command::new("callgrind_annotate")
        .arg("--tree=caller")
        .arg("--inclusive=yes")
        .arg("--threshold=100")
        .arg(file)
        .output()
        .map_err(|e| format!("running callgrind_annotate: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let per_step = |cost: u64| cost as f64 / steps as f64;
    let mut report = String::new();
    // A block is the function's callers, one line each, then the function.
    for block in String::from_utf8_lossy(&output.stdout).split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        let Some(own) = lines.iter().find(|line| line.contains("*  ")) else {
            continue;
        };
        let (Some(total), Some(name)) = (count(own), function(own)) else {
            continue;
        };
        // The file as well as the name, so a pattern can name the allocator's
        // `malloc.c:malloc` apart from everything else called `malloc`.
        if !own.contains(pattern) || total == 0 {
            continue;
        }
        report.push_str(&format!("{:>9.1}  {name}\n", per_step(total)));
        let mut from: Vec<(u64, String, String)> = lines
            .iter()
            .filter(|line| line.contains(" < "))
            .filter_map(|line| {
                let cost = count(line)?;
                let calls = line
                    .rsplit_once(" (")
                    .map(|(_, calls)| calls.split_once(')').map_or(calls, |(n, _)| n))
                    .filter(|calls| calls.ends_with('x'))
                    .unwrap_or("")
                    .to_owned();
                Some((cost, function(line)?, calls))
            })
            .filter(|(cost, ..)| *cost * 100 >= total)
            .collect();
        from.sort_by_key(|caller| std::cmp::Reverse(caller.0));
        for (cost, caller, calls) in from {
            report.push_str(&format!("{:>9.1}    < {caller} {calls}\n", per_step(cost)));
        }
    }
    Ok(report)
}

/// A count in millions, as a person reads one.
fn millions(count: u64) -> String {
    format!("{:.1}M", count as f64 / 1e6)
}
