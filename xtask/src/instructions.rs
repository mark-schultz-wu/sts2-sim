//! What one replayed step costs, in instructions, under callgrind.
//!
//! An instruction count is exact and repeatable, so two builds compare
//! without the noise a timer carries. It says nothing about caches or branch
//! prediction, which is what the criterion benches are for.
//!
//! The replay is `sts2-golden`'s `replay_sites` example, stepping through the
//! rules alone. Nothing inside map generation is counted: callgrind stops
//! counting as the builder is entered and starts again as it returns. Every
//! run generates its maps once, and a search inside a fight never does. So
//! every figure here, a caller's inclusive one among them, leaves it out.
//!
//! Each profile is kept under `target/profile/<label>.callgrind`, so a later
//! run can be held against it with `--vs`. `--top` lists the simulator's
//! costliest functions, or, beside `--vs`, the ones that moved most.
//!
//! ```text
//! cargo xtask instructions before
//! cargo xtask instructions after --vs before --top 10
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Map generation, as callgrind names it. A closure inside it ends in `}`
/// rather than `>`, so this names the builder alone.
const MAP: &str = "*ActMapBuilder*>::generate::<*>";

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

pub fn run(args: &[String]) -> Result<String, String> {
    let mut label = None;
    let mut vs = None;
    let mut top = 0;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--vs" => vs = Some(args.next().ok_or("--vs names a label")?.clone()),
            "--top" => {
                top = args
                    .next()
                    .ok_or("--top names a count")?
                    .parse()
                    .map_err(|e| format!("--top: {e}"))?;
            }
            other if label.is_none() && !other.starts_with("--") => label = Some(other.to_owned()),
            other => return Err(format!("unexpected argument '{other}'")),
        }
    }
    let label = label.unwrap_or_else(|| "current".to_owned());

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
        // comes after it.
        .arg(format!("--toggle-collect={MAP}"))
        .arg("--collect-atstart=yes")
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
        "{label}: {steps} steps, {} outside map generation, {:.0} per step\n",
        millions(current.total),
        current.per_step(),
    );
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
fn workspace() -> Result<PathBuf, String> {
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
fn divide_by(stdout: &str) -> Result<u64, String> {
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

/// A count in millions, as a person reads one.
fn millions(count: u64) -> String {
    format!("{:.1}M", count as f64 / 1e6)
}
