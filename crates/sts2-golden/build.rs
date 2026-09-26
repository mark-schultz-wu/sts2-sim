//! Fills the oracle cache before the tests that read it are built.
//!
//! Holding the simulator against the real game means knowing what the real game
//! said. Asking it costs a game launch, and the answer never changes for a
//! given recording, build and mod. So the answers are taken once and kept.
//!
//! Cargo decides when this runs, through the `rerun-if` lines below, and runs
//! it once with nothing else beside it. That is the whole of the locking this
//! needs: two builds cannot fill the same cache at once.
//!
//! The cache sits under the target directory, so `cargo clean` takes it, keyed
//! by content. A change to any part of that key is a miss.
//!
//! No game means no filling and no failure here. The tests say what is absent,
//! which is a better place to say it than a build.

use std::path::{Path, PathBuf};

include!("src/suite.rs");

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{ContentVersion, Play};
use sts2_headless::{
    Cached, Config, FromTranscript, MOD_IDENTITY, Oracle, game_dir_from_env, recording,
};

type V = V0_107_1;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=tests/golden");
    println!("cargo::rerun-if-changed=tests/scenarios");
    println!("cargo::rerun-if-env-changed=STS2_GAME_DIR");
    println!("cargo::rerun-if-env-changed=STS2_GAME_VERSION");

    let cache = cache_dir();
    std::fs::create_dir_all(&cache).expect("the cache directory is creatable");
    println!("cargo::rustc-env=STS2_ORACLE_CACHE={}", cache.display());

    // One game answers every miss. `start` restarts the run in place, so a
    // launch is paid once rather than once for each recording.
    let mut game = None;
    for (name, recording) in every_recording() {
        if let Err(why) = ensure(&name, &recording, &cache, &mut game) {
            // A warning and not a failure. Somebody without the game vendored
            // still builds and still runs everything that does not need it.
            println!("cargo::warning=no oracle transcript for {name}: {why}");
        }
    }
}

/// `<target>/sts2-oracle`, wherever the target directory turns out to be.
///
/// `OUT_DIR` is `<target>/<profile>/build/<package>-<hash>/out`, so the target
/// directory is four above it. A cross build puts the triple in the middle and
/// this lands one deeper, which costs a second copy of the cache and nothing
/// else.
fn cache_dir() -> PathBuf {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let target = out
        .ancestors()
        .nth(4)
        .expect("OUT_DIR is four below the target directory");
    target.join("sts2-oracle")
}

/// Every run directory, in a fixed order.
/// Every recording a transcript is wanted for: the long runs, then the
/// scenarios.
///
/// A scenario is one file under `tests/scenarios/`. See `src/suite.rs`.
fn every_recording() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = runs()
        .into_iter()
        .map(|run| {
            let name = run
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let source = run.join("run.jsonl");
            let text = std::fs::read_to_string(&source)
                .unwrap_or_else(|e| panic!("reading {}: {e}", source.display()));
            (name, text)
        })
        .collect();

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios");
    let written = read_suite(&dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()));
    // A scenario file carries no build stamp. One goes in front of it here, so
    // the recording reader can still refuse a build it does not describe.
    let stamp =
        serde_json::to_string(&sts2_headless::Stamp::of(&V::VERSION)).expect("a stamp is json");
    out.extend(
        written
            .into_iter()
            .map(|w| (w.name, format!("{stamp}\n{}", w.commands))),
    );
    out
}

fn runs() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut runs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    runs.sort();
    runs
}

/// Makes the transcript for this run current, or says why it is not.
///
/// The key is the recording, hashed, and the mod that answered.
///
/// The build needs no part of it. A recording carries the build it was taken
/// on, and `recording::read` refuses one that does not describe the tables. So
/// one recording cannot be answered by two builds.
fn ensure(
    name: &str,
    recording: &str,
    cache: &Path,
    game: &mut Option<Oracle<V>>,
) -> Result<(), String> {
    let bytes = recording.as_bytes();
    let key = format!("{:016x} {MOD_IDENTITY}", fnv1a(bytes));

    let transcript = cache.join(format!("{name}.jsonl"));
    let stamped = cache.join(format!("{name}.key"));
    if std::fs::read_to_string(&stamped).is_ok_and(|held| held.trim() == key)
        && transcript.is_file()
    {
        return Ok(());
    }

    let recorded = recording::read::<V>(std::io::Cursor::new(bytes))
        .map_err(|e| format!("reading the recording for {name}: {e}"))?;

    let dir = game_dir_from_env().ok_or("STS2_GAME_DIR is unset; see .envrc")?;
    println!(
        "cargo::warning=asking the game for {name} ({} steps)",
        recorded.len()
    );

    // Rewritten from nothing rather than grown, so a stale transcript cannot
    // contribute a prefix to a fresh one.
    let _ = std::fs::remove_file(&transcript);
    let _ = std::fs::remove_file(&stamped);

    // The start goes first and is not a step. `Play::begin` takes it, so
    // `steps()` does not carry it, and a game driven from `steps()` alone
    // plays whatever character it launched in.
    // Taken, and put back only once the recording is filled. A game that
    // failed partway is dropped, and the next miss launches a fresh one.
    let oracle = match game.take() {
        Some(oracle) => oracle,
        None => Oracle::<V>::launch(Config::new(dir)).map_err(|e| format!("launching: {e}"))?,
    };
    let mut cached = Cached::begin(
        FromTranscript {
            path: transcript.clone(),
            inner: Some(oracle),
        },
        recorded.start(),
    )
    .map_err(|e| format!("starting {name}: {e}"))?;
    cached
        .fill(recorded.steps())
        .map_err(|e| format!("replaying {name}: {e}"))?;
    *game = cached.into_inner();

    std::fs::write(&stamped, key).map_err(|e| format!("writing {}: {e}", stamped.display()))?;
    Ok(())
}

/// FNV-1a, so the key means the same thing next year.
///
/// A hash written down rather than borrowed. The default hasher's answer is
/// not promised to stay put across releases, and the key is the whole of the
/// invalidation.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}
