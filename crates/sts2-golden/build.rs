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
    println!("cargo::rerun-if-env-changed=STS2_ORACLE_STORE");
    println!("cargo::rerun-if-env-changed=STS2_ORACLE_WIDTH");

    let cache = cache_dir();
    std::fs::create_dir_all(&cache).expect("the cache directory is creatable");
    println!("cargo::rustc-env=STS2_ORACLE_CACHE={}", cache.display());
    let store = std::env::var_os("STS2_ORACLE_STORE").map(PathBuf::from);
    if let Some(store) = &store {
        std::fs::create_dir_all(store).expect("the oracle store is creatable");
    }

    // A few games share the misses, each taking the next recording as it
    // frees up. `start` restarts the run in place, so each game's launch is
    // paid once rather than once for each recording. A mod change misses
    // every recording, and one game alone answers them in half an hour.
    let recordings = every_recording();
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for worker in 0..width() {
            let home = cache.join("homes").join(format!("worker{worker}"));
            let (recordings, next, cache, store) = (&recordings, &next, &cache, &store);
            scope.spawn(move || {
                let mut game = None;
                while let Some((name, recording)) =
                    recordings.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
                {
                    let ensured =
                        ensure(name, recording, cache, store.as_deref(), &home, &mut game);
                    if let Err(why) = ensured {
                        // A warning and not a failure. Somebody without the
                        // game vendored still builds and still runs everything
                        // that does not need it.
                        println!("cargo::warning=no oracle transcript for {name}: {why}");
                    }
                }
            });
        }
    });
}

/// How many games answer the misses at once: `STS2_ORACLE_WIDTH`, or half
/// the machine's threads up to sixteen.
fn width() -> usize {
    std::env::var("STS2_ORACLE_WIDTH")
        .ok()
        .and_then(|width| width.parse().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(1, |threads| (threads.get() / 2).min(16))
        })
        .max(1)
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
///
/// A miss here goes to `store` first. See [`Stored`].
fn ensure(
    name: &str,
    recording: &str,
    cache: &Path,
    store: Option<&Path>,
    home: &Path,
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

    let stored = store.map(|store| Stored::new(store, name, fnv1a(bytes)));
    if let Some(stored) = &stored
        && stored.take(&transcript)
    {
        return std::fs::write(&stamped, key)
            .map_err(|e| format!("writing {}: {e}", stamped.display()));
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

    // Taken, and put back only once the recording is filled. A game that
    // failed partway is dropped, and the next miss launches a fresh one.
    let oracle = match game.take() {
        Some(oracle) => oracle,
        None => {
            let mut config = Config::new(dir);
            // A profile of its own, since games that share one race on its
            // saves.
            config.data_home = Some(home.to_path_buf());
            Oracle::<V>::launch(config).map_err(|e| format!("launching: {e}"))?
        }
    };
    // The start goes first and is not a step. `Play::begin` takes it, so
    // `steps()` does not carry it, and a game driven from `steps()` alone
    // plays whatever character it launched in.
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

    if let Some(stored) = &stored {
        stored.put(&transcript)?;
    }
    std::fs::write(&stamped, key).map_err(|e| format!("writing {}: {e}", stamped.display()))?;
    Ok(())
}

/// One transcript's place in a store that outlives the target directory.
///
/// CI starts every job with an empty target directory. A store is a directory
/// the job mounts, named by `STS2_ORACLE_STORE`, which keeps what earlier jobs
/// recorded. A miss is looked for there before the game is asked, and what the
/// game answers goes into it.
struct Stored {
    path: PathBuf,
}

impl Stored {
    /// Named by the whole key, so a transcript in the store never changes. Two
    /// branches with different versions of one scenario keep one file each.
    fn new(store: &Path, name: &str, recording: u64) -> Self {
        Self {
            path: store.join(format!("{name}.{recording:016x}.{MOD_IDENTITY}.jsonl")),
        }
    }

    /// Copies the stored transcript to `transcript`, and says whether there
    /// was one. A hit moves the file's time, so pruning by age keeps the
    /// transcripts still in use.
    fn take(&self, transcript: &Path) -> bool {
        if std::fs::copy(&self.path, transcript).is_err() {
            return false;
        }
        if let Ok(file) = std::fs::File::options().append(true).open(&self.path) {
            let _ = file.set_modified(std::time::SystemTime::now());
        }
        true
    }

    /// Puts `transcript` in the store: a copy beside the name, then a rename
    /// onto it.
    ///
    /// So jobs that run at once can share a store. Two jobs that both record a
    /// miss write two answers that are equally good.
    fn put(&self, transcript: &Path) -> Result<(), String> {
        let partial = self
            .path
            .with_extension(format!("{}.partial", std::process::id()));
        std::fs::copy(transcript, &partial)
            .and_then(|_| std::fs::rename(&partial, &self.path))
            .map_err(|e| format!("storing {}: {e}", self.path.display()))
    }
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
