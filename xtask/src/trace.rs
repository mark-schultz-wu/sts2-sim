//! Where a replayed step's instructions go, span by span.
//!
//! `record` builds `sts2-trace` with the `profiling` profile and runs it. It
//! replays the recorded run under a `tracing` layer that counts instructions.
//! It writes folded stacks: one line per span path, root first, with what
//! that path's own span ran. `show` reads that back as a
//! tree in instructions per step, and `--callers` turns it the other way up.
//! `svg` draws it as a flamegraph.
//!
//! Map generation is kept apart, since a search inside a fight never runs it.
//! `--map` reads it instead of the rest, per map built rather than per step.
//!
//! ```text
//! cargo xtask trace record before
//! cargo xtask trace show before --root notify --depth 3
//! cargo xtask trace show after --vs before --root turn::end
//! cargo xtask trace show after --callers modified_by
//! cargo xtask trace svg after
//! cargo xtask trace show after --map --depth 3
//! ```
//!
//! A build with live spans inlines a little differently from a release build,
//! which puts its total a few per cent off. `instructions --vs` is the exact
//! check of what a change did.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{self, FileError, ProcessError};

/// Where `show` starts when no root is named: the replays themselves.
const REPLAY: &str = "sts2_trace::replay";

/// Where `show --map` starts when no root is named: each map built.
const MAP: &str = "act_map";

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    command: Trace,
}

#[derive(clap::Subcommand)]
enum Trace {
    /// Build `sts2-trace`, replay the recorded run under it, and keep what it
    /// folded.
    Record {
        /// What to call the profile.
        #[arg(default_value = "current")]
        label: String,
    },
    /// A recorded profile as a tree, in instructions per step.
    Show(Show),
    /// A recorded profile as a flamegraph, next to its folded file.
    Svg(Stacks),
}

/// Which recorded stacks to read.
#[derive(clap::Args)]
struct Stacks {
    /// The profile's label.
    #[arg(default_value = "current")]
    label: String,
    /// Map generation, per map built, rather than everything else.
    #[arg(long)]
    map: bool,
}

/// What `show` takes beside the stacks it reads.
#[derive(clap::Args)]
struct Show {
    #[command(flatten)]
    stacks: Stacks,
    /// The span the tree starts at.
    #[arg(long)]
    root: Option<String>,
    /// How many frames deep the tree goes.
    #[arg(long, default_value_t = 4)]
    depth: usize,
    /// The share of the root, in per cent, a frame needs to be listed.
    #[arg(long, default_value_t = 1.0)]
    min: f64,
    /// Another profile's label, to show this one against.
    #[arg(long)]
    vs: Option<String>,
    /// The spans that call these, rather than what they call.
    #[arg(long)]
    callers: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error("{0}; record it first")]
    NotRecorded(FileError),
    #[error("{} is empty", path.display())]
    Empty { path: PathBuf },
    #[error("{} carries no {key}", path.display())]
    NoHeaderKey { path: PathBuf, key: &'static str },
    #[error("{line}: {source}")]
    Line {
        line: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("drawing the flamegraph: {0}")]
    Flamegraph(#[source] std::io::Error),
}

pub fn run(args: Args) -> Result<String, Error> {
    let dir = crate::workspace().join("target/profile");
    error::create_dir(&dir)?;
    match args.command {
        Trace::Record { label } => record(&dir, &label),
        Trace::Show(mut show_args) => {
            let label = stacked(&show_args.stacks);
            // The map's stacks sit in a file of their own beside the rest.
            if show_args.stacks.map {
                show_args.vs = show_args.vs.map(|vs| format!("{vs}.map"));
            }
            show(&dir, &label, &show_args)
        }
        Trace::Svg(stacks) => svg(&dir, &stacked(&stacks), stacks.map),
    }
}

/// The label of the file that holds these stacks.
fn stacked(stacks: &Stacks) -> String {
    match stacks.map {
        true => format!("{}.map", stacks.label),
        false => stacks.label.clone(),
    }
}

fn folded_path(dir: &Path, label: &str) -> PathBuf {
    dir.join(format!("{label}.folded"))
}

/// Build `sts2-trace`, run it, and keep what it folded.
fn record(dir: &Path, label: &str) -> Result<String, Error> {
    let root = crate::workspace();
    error::status(
        Command::new("cargo")
            .args(["build", "-q", "--profile", "profiling", "-p", "sts2-trace"])
            .current_dir(&root),
    )?;
    let file = folded_path(dir, label);
    let traced = error::output(
        Command::new(root.join("target/profiling/sts2-trace"))
            .arg(&file)
            .current_dir(&root),
    )?;
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
fn read(dir: &Path, label: &str, callers: Option<&str>) -> Result<(Node, f64), Error> {
    let file = folded_path(dir, label);
    let text = error::read(&file).map_err(Error::NotRecorded)?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| Error::Empty { path: file.clone() })?;
    let number = |key: &'static str| -> Result<f64, Error> {
        header
            .split_whitespace()
            .skip_while(|word| *word != key)
            .nth(1)
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| Error::NoHeaderKey {
                path: file.clone(),
                key,
            })
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
        let parse = |n: &str| {
            n.parse::<u64>().map_err(|source| Error::Line {
                line: line.to_owned(),
                source,
            })
        };
        let (samples, calls) = (parse(samples)?, parse(calls)?);
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
fn show(dir: &Path, label: &str, options: &Show) -> Result<String, Error> {
    let (depth, min) = (options.depth, options.min);
    let focus = |label: &str| -> Result<(Node, f64), Error> {
        if let Some(pattern) = &options.callers {
            return read(dir, label, Some(pattern));
        }
        let (all, worth) = read(dir, label, None)?;
        let mut zoomed = Node::default();
        let start = match options.stacks.map {
            true => MAP,
            false => REPLAY,
        };
        all.zoom(options.root.as_deref().unwrap_or(start), &mut zoomed);
        Ok((zoomed, worth))
    };
    let (now, worth) = focus(label)?;
    let then = options.vs.as_deref().map(focus).transpose()?;
    let root = match (&options.callers, &options.root) {
        (Some(pattern), _) => format!("what calls {pattern}"),
        (None, Some(pattern)) => pattern.clone(),
        (None, None) if options.stacks.map => "map generation".to_owned(),
        (None, None) => "the replay".to_owned(),
    };
    let per = match options.stacks.map {
        true => "map",
        false => "step",
    };
    let mut out = match &then {
        None => format!(
            "{label}, {root}: {:.0} per {per}{}\n  per {per}  share  per call  span\n",
            now.total as f64 * worth,
            match now.calls {
                0 => String::new(),
                calls => format!(", {:.0} per call", now.total as f64 / calls as f64),
            }
        ),
        Some((then, then_worth)) => format!(
            "{label} against {}, {root}: {:.0} -> {:.0} per {per}\n  before     after   change  span\n",
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
fn svg(dir: &Path, label: &str, map: bool) -> Result<String, Error> {
    let file = folded_path(dir, label);
    let text = error::read(&file).map_err(Error::NotRecorded)?;
    // A flamegraph takes a stack and a count, so the calls are left off.
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.rsplit_once(' ').map(|(stack, _)| stack))
        .collect();
    let out = dir.join(format!("{label}.svg"));
    let writer = error::create(&out)?;
    let mut options = inferno::flamegraph::Options::default();
    options.title = match map {
        true => format!("{label}: map generation"),
        false => format!("{label}: a replayed step, outside map generation"),
    };
    options.count_name = "instructions".into();
    inferno::flamegraph::from_lines(&mut options, lines, std::io::BufWriter::new(writer))
        .map_err(Error::Flamegraph)?;
    Ok(format!("drawn into {}\n", out.display()))
}
