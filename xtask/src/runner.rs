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
use std::time::{Duration, Instant};

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

/// How often [`pool`] says how far it is.
const PROGRESS_EVERY: Duration = Duration::from_secs(5 * 60);

/// `work` on every job, `width` at a time, each told its worker's number and
/// the job's index. What each came to, in the jobs' order.
///
/// Every [`PROGRESS_EVERY`], as a job ends, a line on stderr says how many
/// are done and about how long the rest will take.
pub fn pool<J: Sync, R: Send>(
    jobs: &[J],
    width: usize,
    work: impl Fn(usize, usize, &J) -> R + Sync,
) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let done = Mutex::new(Vec::with_capacity(jobs.len()));
    let started = Instant::now();
    let said = Mutex::new(started);
    std::thread::scope(|scope| {
        for worker in 0..width.max(1) {
            let (next, done, work, said) = (&next, &done, &work, &said);
            scope.spawn(move || {
                while let Some(at) =
                    Some(next.fetch_add(1, Ordering::Relaxed)).filter(|&at| at < jobs.len())
                {
                    let result = work(worker, at, &jobs[at]);
                    let finished = {
                        let mut done = done.lock().expect("no thread panics");
                        done.push((at, result));
                        done.len()
                    };
                    let mut said = said.lock().expect("no thread panics");
                    if said.elapsed() >= PROGRESS_EVERY {
                        *said = Instant::now();
                        eprintln!("{}", progress(finished, jobs.len(), started.elapsed()));
                    }
                }
            });
        }
    });
    let mut done = done.into_inner().expect("no thread panics");
    done.sort_by_key(|(at, _)| *at);
    done.into_iter().map(|(_, result)| result).collect()
}

/// How far a pool is: `done` of `total` jobs after `elapsed`, and the rest
/// at the rate so far.
fn progress(done: usize, total: usize, elapsed: Duration) -> String {
    let minutes = elapsed.as_secs() / 60;
    let left = elapsed.mul_f64((total - done) as f64 / done.max(1) as f64);
    format!(
        "{done} of {total} runs done after {minutes} min, about {} min left",
        left.as_secs().div_ceil(60)
    )
}

/// Seed `n`, as the runs spell it: `C` and `n` in nine digits.
pub fn seed(n: usize) -> String {
    format!("C{n:09}")
}

/// `args`, as the runner's arguments.
pub fn args<I: IntoIterator<Item = S>, S: Into<OsString>>(args: I) -> Vec<OsString> {
    args.into_iter().map(Into::into).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_says_the_rest_at_the_rate_so_far() {
        assert_eq!(
            progress(750, 3000, Duration::from_secs(30 * 60)),
            "750 of 3000 runs done after 30 min, about 90 min left"
        );
    }

    #[test]
    fn progress_rounds_what_is_left_up() {
        assert_eq!(
            progress(2999, 3000, Duration::from_secs(60 * 60)),
            "2999 of 3000 runs done after 60 min, about 1 min left"
        );
    }
}
