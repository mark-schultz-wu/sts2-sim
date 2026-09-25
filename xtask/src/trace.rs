//! Where a replayed step's instructions go, span by span.
//!
//! `record` builds `sts2-trace` with the `profiling` profile and runs it. It
//! replays the recorded run under a `tracing` layer that counts instructions.
//! It writes folded stacks: one line per span path, root first, with what
//! that path's own span ran. `show` reads that back as a
//! tree in instructions per step, and `--callers` turns it the other way up.
//! `svg` draws it as a flamegraph.
//!
//! ```text
//! cargo xtask trace record before
//! cargo xtask trace show before --root notify --depth 3
//! cargo xtask trace show after --vs before --root turn::end
//! cargo xtask trace show after --callers modified_by
//! cargo xtask trace svg after
//! ```
//!
//! A build with live spans inlines a little differently from a release build,
//! which puts its total a few per cent off. `instructions --vs` is the exact
//! check of what a change did.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where `show` starts when no root is named: the replays themselves.
const REPLAY: &str = "sts2_trace::replay";

pub fn run(args: &[String]) -> Result<String, String> {
    let (command, rest) = args
        .split_first()
        .ok_or("usage: cargo xtask trace <record|show|svg> <label> ...")?;
    let dir = workspace()?.join("target/profile");
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let mut options = Options::parse(rest)?;
    let label = options.label.take().unwrap_or_else(|| "current".to_owned());
    match command.as_str() {
        "record" => record(&dir, &label),
        "show" => show(&dir, &label, &options),
        "svg" => svg(&dir, &label),
        other => Err(format!(
            "unknown trace command '{other}'; try: record, show, svg"
        )),
    }
}

/// What `show` and `record` take beside the label.
#[derive(Default)]
struct Options {
    label: Option<String>,
    root: Option<String>,
    depth: Option<usize>,
    min: Option<f64>,
    vs: Option<String>,
    callers: Option<String>,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Options::default();
        let mut args = args.iter();
        let value = |flag: &str, args: &mut std::slice::Iter<'_, String>| {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{flag} takes a value"))
        };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--root" => options.root = Some(value("--root", &mut args)?),
                "--depth" => options.depth = Some(parse(&value("--depth", &mut args)?, "--depth")?),
                "--min" => options.min = Some(parse(&value("--min", &mut args)?, "--min")?),
                "--vs" => options.vs = Some(value("--vs", &mut args)?),
                "--callers" => options.callers = Some(value("--callers", &mut args)?),
                other if options.label.is_none() && !other.starts_with("--") => {
                    options.label = Some(other.to_owned())
                }
                other => return Err(format!("unexpected argument '{other}'")),
            }
        }
        Ok(options)
    }
}

fn parse<T: std::str::FromStr>(text: &str, flag: &str) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    text.parse().map_err(|e| format!("{flag}: {e}"))
}

/// The repository's root, which is where cargo and the profiles live.
fn workspace() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask sits inside the workspace".into())
}

fn folded_path(dir: &Path, label: &str) -> PathBuf {
    dir.join(format!("{label}.folded"))
}

/// Build `sts2-trace`, run it, and keep what it folded.
fn record(dir: &Path, label: &str) -> Result<String, String> {
    let root = workspace()?;
    let built = Command::new("cargo")
        .args(["build", "-q", "--profile", "profiling", "-p", "sts2-trace"])
        .current_dir(&root)
        .status()
        .map_err(|e| format!("running cargo: {e}"))?;
    if !built.success() {
        return Err("sts2-trace did not build".into());
    }
    let file = folded_path(dir, label);
    let traced = Command::new(root.join("target/profiling/sts2-trace"))
        .arg(&file)
        .current_dir(&root)
        .output()
        .map_err(|e| format!("running sts2-trace: {e}"))?;
    if !traced.status.success() {
        return Err(format!(
            "sts2-trace failed:\n{}",
            String::from_utf8_lossy(&traced.stderr)
        ));
    }
    Ok(format!(
        "{}folded into {}\n",
        String::from_utf8_lossy(&traced.stdout),
        file.display()
    ))
}

/// One span and everything that ran under it, in the folded file's units.
#[derive(Default)]
struct Node {
    total: u64,
    /// How many times the span at this path ran, and nothing for the root.
    calls: u64,
    children: BTreeMap<String, Node>,
}

impl Node {
    fn add(&mut self, frames: &[&str], samples: u64, calls: u64) {
        self.total += samples;
        match frames.split_first() {
            Some((first, rest)) => self
                .children
                .entry((*first).to_owned())
                .or_default()
                .add(rest, samples, calls),
            None => self.calls += calls,
        }
    }

    fn own(&self) -> u64 {
        self.total - self.children.values().map(|child| child.total).sum::<u64>()
    }

    /// Every frame whose name holds `pattern`, merged into one, and not
    /// looked inside again once found. That is what zooming into a frame of
    /// a flamegraph shows, wherever the frame ran.
    fn zoom(&self, pattern: &str, into: &mut Node) {
        for (name, child) in &self.children {
            if name.contains(pattern) {
                into.merge(child);
            } else {
                child.zoom(pattern, into);
            }
        }
    }

    fn merge(&mut self, other: &Node) {
        self.total += other.total;
        self.calls += other.calls;
        for (name, child) in &other.children {
            self.children.entry(name.clone()).or_default().merge(child);
        }
    }
}

/// A folded file, read back as a tree, with what one sample is worth in
/// instructions per step.
///
/// With `callers`, the tree is of what called each frame whose name holds
/// it, the nearest caller first. A frame that calls itself counts once, at
/// its outermost call.
fn read(dir: &Path, label: &str, callers: Option<&str>) -> Result<(Node, f64), String> {
    let file = folded_path(dir, label);
    let text = std::fs::read_to_string(&file)
        .map_err(|e| format!("reading {}: {e}; record it first", file.display()))?;
    let mut lines = text.lines();
    let header = lines.next().ok_or("an empty folded file")?;
    let number = |key: &str| -> Result<f64, String> {
        header
            .split_whitespace()
            .skip_while(|word| *word != key)
            .nth(1)
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| format!("{} carries no {key}", file.display()))
    };
    let worth = number("period")? / number("steps")?;
    let mut root = Node::default();
    for line in lines {
        // The stack, what its own span ran, and how many times it ran.
        let mut words = line.rsplitn(3, ' ');
        let (Some(calls), Some(samples), Some(stack)) = (words.next(), words.next(), words.next())
        else {
            continue;
        };
        let samples: u64 = samples.parse().map_err(|e| format!("{line}: {e}"))?;
        let calls: u64 = calls.parse().map_err(|e| format!("{line}: {e}"))?;
        let frames: Vec<&str> = stack.split(';').collect();
        match callers {
            None => root.add(&frames, samples, calls),
            Some(pattern) => {
                if let Some(at) = frames.iter().position(|frame| frame.contains(pattern)) {
                    let above: Vec<&str> = frames[..at].iter().rev().copied().collect();
                    root.add(&above, samples, 0);
                }
            }
        }
    }
    Ok((root, worth))
}

/// The tree under the root `options` names, one frame a line.
fn show(dir: &Path, label: &str, options: &Options) -> Result<String, String> {
    let depth = options.depth.unwrap_or(4);
    let min = options.min.unwrap_or(1.0);
    let focus = |label: &str| -> Result<(Node, f64), String> {
        if let Some(pattern) = &options.callers {
            return read(dir, label, Some(pattern));
        }
        let (all, worth) = read(dir, label, None)?;
        let mut zoomed = Node::default();
        all.zoom(options.root.as_deref().unwrap_or(REPLAY), &mut zoomed);
        Ok((zoomed, worth))
    };
    let (now, worth) = focus(label)?;
    let then = options.vs.as_deref().map(focus).transpose()?;
    let root = match (&options.callers, &options.root) {
        (Some(pattern), _) => format!("what calls {pattern}"),
        (None, Some(pattern)) => pattern.clone(),
        (None, None) => "the replay".to_owned(),
    };
    let mut out = match &then {
        None => format!(
            "{label}, {root}: {:.0} per step{}\n  per step  share  per call  span\n",
            now.total as f64 * worth,
            match now.calls {
                0 => String::new(),
                calls => format!(", {:.0} per call", now.total as f64 / calls as f64),
            }
        ),
        Some((then, then_worth)) => format!(
            "{label} against {}, {root}: {:.0} -> {:.0} per step\n  before     after   change  span\n",
            options.vs.as_deref().unwrap_or_default(),
            then.total as f64 * then_worth,
            now.total as f64 * worth
        ),
    };
    // Shares and the cut-off are of the root, per step.
    let base = (now.total.max(1)) as f64 * worth;
    let then = then.as_ref().map(|(node, worth)| (node, *worth));
    lines(&mut out, &now, worth, then, base, min, depth, 0);
    Ok(out)
}

/// One line per child frame, costliest first, and the frame's own cost last.
///
/// `base` is the root's cost per step, and every share is of it. A frame under
/// `min` per cent of it on both sides is left out.
#[allow(clippy::too_many_arguments)]
fn lines(
    out: &mut String,
    now: &Node,
    worth: f64,
    then: Option<(&Node, f64)>,
    base: f64,
    min: f64,
    depth: usize,
    level: usize,
) {
    if level == depth {
        return;
    }
    let empty = Node::default();
    let cost = |node: Option<&Node>, worth: f64| node.map_or(0, |node| node.total) as f64 * worth;
    let mut names: Vec<&String> = now.children.keys().collect();
    if let Some((then, _)) = then {
        names.extend(
            then.children
                .keys()
                .filter(|name| !now.children.contains_key(*name)),
        );
    }
    let before = |name: &String| {
        then.map(|(then, then_worth)| (then.children.get(name).unwrap_or(&empty), then_worth))
    };
    let widest = |name: &String| {
        let after = cost(now.children.get(name), worth);
        before(name).map_or(after, |(node, then_worth)| {
            after.max(cost(Some(node), then_worth))
        })
    };
    names.sort_by(|a, b| widest(b).total_cmp(&widest(a)));
    let indent = "  ".repeat(level);
    for name in names {
        if widest(name) / base * 100.0 < min {
            continue;
        }
        let child = now.children.get(name).unwrap_or(&empty);
        let after = cost(Some(child), worth);
        match before(name) {
            // A recording counts every instruction, so a span's own total
            // over its calls is what one call cost.
            None => {
                let per_call = match child.calls {
                    0 => String::new(),
                    calls => format!("{:.0}", child.total as f64 / calls as f64),
                };
                writeln!(
                    out,
                    "  {after:>8.0} {:>5.1}%  {per_call:>8}  {indent}{name}",
                    after / base * 100.0
                )
            }
            Some((node, then_worth)) => {
                let then_cost = cost(Some(node), then_worth);
                writeln!(
                    out,
                    "  {then_cost:>8.0} {after:>8.0} {:>+8.0}  {indent}{name}",
                    after - then_cost
                )
            }
        }
        .unwrap();
        lines(out, child, worth, before(name), base, min, depth, level + 1);
    }
    let own = now.own() as f64 * worth;
    let then_own = then.map(|(then, then_worth)| then.own() as f64 * then_worth);
    if own.max(then_own.unwrap_or(0.0)) / base * 100.0 < min {
        return;
    }
    match then_own {
        None => writeln!(
            out,
            "  {own:>8.0} {:>5.1}%            {indent}(self)",
            own / base * 100.0
        ),
        Some(then_own) => writeln!(
            out,
            "  {then_own:>8.0} {own:>8.0} {:>+8.0}  {indent}(self)",
            own - then_own
        ),
    }
    .unwrap();
}

/// Draw a recorded profile as a flamegraph, next to its folded file.
fn svg(dir: &Path, label: &str) -> Result<String, String> {
    let file = folded_path(dir, label);
    let text = std::fs::read_to_string(&file)
        .map_err(|e| format!("reading {}: {e}; record it first", file.display()))?;
    // A flamegraph takes a stack and a count, so the calls are left off.
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.rsplit_once(' ').map(|(stack, _)| stack))
        .collect();
    let out = dir.join(format!("{label}.svg"));
    let writer =
        std::fs::File::create(&out).map_err(|e| format!("creating {}: {e}", out.display()))?;
    let mut options = inferno::flamegraph::Options::default();
    options.title = format!("{label}: a replayed step, outside map generation");
    options.count_name = "instructions".into();
    inferno::flamegraph::from_lines(&mut options, lines, std::io::BufWriter::new(writer))
        .map_err(|e| format!("drawing the flamegraph: {e}"))?;
    Ok(format!("drawn into {}\n", out.display()))
}
