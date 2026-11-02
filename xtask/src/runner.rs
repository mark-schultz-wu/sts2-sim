//! Running `sts2-run`, the one runner, from the commands that start runs.
//!
//! Each run is a process of its own, which ends on one verdict line. A run
//! whose game went away is played again. A pool plays many runs at once.
//! `docs/RUNNERS.md` says what the runner does.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use sts2_core::{End, Verdict};

/// How many times a run whose game went away is played.
pub const TRIES: usize = 3;

/// The build a game run plays: `STS2_GAME_VERSION`'s, as the game's tools
/// read it.
pub fn game_build() -> String {
    std::env::var("STS2_GAME_VERSION").unwrap_or_else(|_| "v0.107.1".to_owned())
}

/// What one run of the runner left.
pub struct Ran {
    /// The verdict it ended on, or why it did not end on one.
    pub verdict: Result<Verdict, String>,
    /// Everything it printed, its moves among them where it printed them.
    pub stdout: String,
}

/// Runs `runner` with `args` and `env`, once.
pub fn once(runner: &Path, args: &[OsString], env: &[(&str, OsString)]) -> Ran {
    let output = Command::new(runner)
        .args(args)
        .envs(env.iter().map(|(key, value)| (key, value)))
        .output();
    let output = match output {
        Ok(output) => output,
        Err(e) => {
            return Ran {
                verdict: Err(format!("launching the runner: {e}")),
                stdout: String::new(),
            };
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let verdict = Verdict::last_in(&stdout).ok_or_else(|| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let last = stderr.lines().last().unwrap_or("no word why");
        format!("exited with {:?}: {last}", output.status.code())
    });
    Ran { verdict, stdout }
}

/// [`once`], and again while the run's game went away, up to [`TRIES`]
/// times. Each play again counts in `retried`.
pub fn retrying(
    runner: &Path,
    args: &[OsString],
    env: &[(&str, OsString)],
    retried: &AtomicUsize,
) -> Ran {
    let mut ran = once(runner, args, env);
    for _ in 1..TRIES {
        match &ran.verdict {
            Ok(verdict) if verdict.end == End::GameGone => {}
            _ => break,
        }
        retried.fetch_add(1, Ordering::Relaxed);
        ran = once(runner, args, env);
    }
    ran
}

/// `work` on every job, `width` at a time, each told its worker's number and
/// the job's index. What each came to, in the jobs' order.
pub fn pool<J: Sync, R: Send>(
    jobs: &[J],
    width: usize,
    work: impl Fn(usize, usize, &J) -> R + Sync,
) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let done = Mutex::new(Vec::with_capacity(jobs.len()));
    std::thread::scope(|scope| {
        for worker in 0..width.max(1) {
            let (next, done, work) = (&next, &done, &work);
            scope.spawn(move || {
                while let Some(at) =
                    Some(next.fetch_add(1, Ordering::Relaxed)).filter(|&at| at < jobs.len())
                {
                    let result = work(worker, at, &jobs[at]);
                    done.lock().expect("no thread panics").push((at, result));
                }
            });
        }
    });
    let mut done = done.into_inner().expect("no thread panics");
    done.sort_by_key(|(at, _)| *at);
    done.into_iter().map(|(_, result)| result).collect()
}

/// Seed `n`, as the runs spell it: `C` and `n` in nine digits.
pub fn seed(n: usize) -> String {
    format!("C{n:09}")
}

/// `args`, as the runner's arguments.
pub fn args<I: IntoIterator<Item = S>, S: Into<OsString>>(args: I) -> Vec<OsString> {
    args.into_iter().map(Into::into).collect()
}
