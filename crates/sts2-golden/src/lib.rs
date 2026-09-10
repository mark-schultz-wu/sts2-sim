//! The simulator, held against the real game.
//!
//! A recording is a seed and a sequence of decisions. The build is
//! deterministic, so driving those decisions into any correct implementation
//! must produce the same observations. Everything here is one half of that
//! comparison or the other. The recordings, what the game answered, and the
//! reporting that says where two answers parted.
//!
//! Its own crate because of what it depends on. Holding the simulator against
//! the game means depending on both, and nothing depends on this. It is also
//! the only crate whose tests need a real game. That is why the build script
//! filling the oracle cache lives here, and nowhere else.

mod counted;

pub use counted::{Allocations, Counted, allocations};

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sts2_headless::{Cached, FromTranscript, Transport};

/// Where the recordings live, one directory per run.
///
/// A directory rather than a stem with several extensions, because a run is one
/// thing. Adding another is dropping another directory in, and the numbers that
/// belong to a run cannot drift onto a different one.
pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Every run, by name, in a fixed order so a failure reads the same twice.
pub mod scenario;
pub mod suite;

pub fn every_run() -> Vec<String> {
    let dir = golden_dir();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no runs in {}", dir.display());
    names
}

/// One run's recording, as the steps that made it.
pub fn recording<V: sts2_core::ContentVersion>(run: &str) -> sts2_core::recording::Recording<V> {
    let path = golden_dir().join(run).join("run.jsonl");
    let file =
        std::fs::File::open(&path).unwrap_or_else(|e| panic!("opening {}: {e}", path.display()));
    sts2_headless::recording::read::<V>(std::io::BufReader::new(file))
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// A game that cannot be launched, which is the only kind a test wants.
///
/// [`Cached`] takes a game to ask when its transcript falls short. A test has
/// no business launching one. Either the transcript covers the recording or the
/// cache was never filled, and a test answers both without a game.
///
/// Uninhabited, so that is a fact about the type and not a promise in a
/// comment.
pub enum NoGame {}

impl Transport for NoGame {
    fn send_line(&mut self, _: &str) -> Result<String, sts2_headless::Error> {
        match *self {}
    }
}

/// What the real game answered this run, as `build.rs` wrote it down.
///
/// Absent when the cache has not been filled, which needs the game vendored and
/// the mod deployed. Every caller here treats that as a failure, because a
/// comparison with nothing to compare against is not a passing test.
pub fn oracle(run: &str) -> Option<Cached<NoGame>> {
    Cached::open(&Path::new(env!("STS2_ORACLE_CACHE")).join(format!("{run}.jsonl")))
}

/// The same, begun on the run it recorded, or an explanation of its absence.
///
/// Starting is the transcript's first entry and not a step. A caller that only
/// opens the cache gets the start's answer for its first decision.
pub fn oracle_or_explain<V: sts2_core::ContentVersion>(
    run: &str,
    start: &sts2_core::Start<V>,
) -> Cached<NoGame> {
    let begun = sts2_core::Play::<V>::begin(
        FromTranscript {
            path: Path::new(env!("STS2_ORACLE_CACHE")).join(format!("{run}.jsonl")),
            inner: None,
        },
        start,
    );
    match begun {
        Ok(cached) => cached,
        Err(e) if oracle(run).is_some() => {
            panic!("{run}: the transcript does not start this run: {e}")
        }
        Err(_) => explain(run),
    }
}

fn explain(run: &str) -> Cached<NoGame> {
    oracle(run).unwrap_or_else(|| {
        panic!(
            "no oracle transcript for {run}.\n\
             `build.rs` writes one by replaying the recording into a real game, \
             which needs the game vendored and the mod deployed:\n    \
             just vendor-game && just deploy && cargo test -p sts2-golden\n\
             It looked in {}",
            env!("STS2_ORACLE_CACHE"),
        )
    })
}

/// The act one map, one node per line.
///
/// The map test generates the same act from the same seed and compares
/// against this. The first step, because that is where a run stands before it
/// has changed anything, and act one is what the generator does so far.
pub fn render_map<V: sts2_core::ContentVersion>(map: &sts2_core::Map<V>) -> String {
    let mut out = String::new();
    for node in map.points.iter() {
        let edges: Vec<String> = node.to.iter().map(ToString::to_string).collect();
        let edges = if edges.is_empty() {
            "-".to_string()
        } else {
            edges.join(" ")
        };
        let _ = writeln!(out, "{} {:?} {edges}", node.at, node.kind);
    }
    out
}

/// One id, by the game's name for it, which is the serde rename.
fn spelled<T: serde::Serialize>(id: &T) -> String {
    match serde_json::to_value(id) {
        Ok(serde_json::Value::String(name)) => name,
        _ => String::new(),
    }
}

/// An option's key as the game spells it, put back together from its parts.
fn option_key<V: sts2_core::ContentVersion>(key: &sts2_core::EventOptionKey<V>) -> String {
    let option = match &key.option {
        sts2_core::EventOptionName::Relic(relic) => spelled(relic),
        sts2_core::EventOptionName::Said(name) => name.clone(),
    };
    format!("{}.pages.INITIAL.options.{option}", spelled(&key.event))
}

/// The room a run opens in, as its ids.
///
/// The first act's ancient, which is Neow. Its options are rolled off the
/// event's own stream, so no counter in an observation says whether a simulator
/// rolled them the same way. This does.
///
/// Prose is left out. The titles come from the game's localisation, and the
/// rules digest does not compare them either.
pub fn render_room<V: sts2_core::ContentVersion>(room: Option<&sts2_core::Room<V>>) -> String {
    let mut out = String::new();
    match room {
        Some(sts2_core::Room::Event {
            event,
            finished,
            options,
            ..
        }) => {
            let _ = writeln!(out, "event {event:?} finished={finished}");
            for option in options {
                let _ = writeln!(
                    out,
                    "option {} {} locked={} proceed={} chosen={} relic={:?}",
                    option.index,
                    option_key(&option.key),
                    option.locked,
                    option.proceed,
                    option.chosen,
                    option.relic
                );
            }
        }
        Some(other) => {
            let _ = writeln!(out, "# not an event: {other:?}");
        }
        None => {
            let _ = writeln!(out, "# no room");
        }
    }
    out
}

/// One file, created or replaced.
pub fn write(path: &Path, body: &str) -> Result<(), String> {
    std::fs::write(path, body).map_err(|e| format!("writing {}: {e}", path.display()))
}
