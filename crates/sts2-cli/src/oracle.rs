//! The oracle as this front end holds it: a daemon, a socket, and a boot that
//! costs twenty seconds.
//!
//! None of that is a fact about a run, and a simulator has none of it. So it is
//! here rather than in [`crate::control`], which drives a game and never learns
//! where one came from.
//!
//! What this does not know is which binary serves. That is the binary's own
//! fact, so [`Held::new`] is handed a way back into it.

use anyhow::{Context, Result};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sts2_core::{Applied, ContentVersion};
use sts2_headless::daemon::{self, DEFAULT_IDLE_TIMEOUT};
use sts2_headless::{Config, Recorded, Session, recording};

use crate::Readable;

/// How to re-enter this front end as the daemon.
///
/// A `std::process::Command` that serves when it is run. Handed in because the
/// subcommand that serves is the binary's, not this module's.
pub type Serving = fn(Duration) -> Result<std::process::Command>;

/// The daemon-held oracle, and every command about the machinery.
pub struct Held<V: ContentVersion> {
    serving: Serving,
    version: PhantomData<V>,
}

impl<V: ContentVersion> Held<V> {
    pub fn new(serving: Serving) -> Held<V> {
        Held {
            serving,
            version: PhantomData,
        }
    }

    /// A handle to whatever the daemon is holding, without starting anything.
    ///
    /// Only `start` brings a game up. Any other command against no game is a
    /// mistake worth naming, not a twenty-second pause followed by a refusal.
    pub fn attached(&self) -> Session<V> {
        Session::new()
    }

    /// Brings the daemon up if it is not already up.
    ///
    /// The daemon comes up. The *game* does not. Launching the game is what
    /// costs twenty seconds, and the daemon only does that when a command needs
    /// one.
    ///
    /// # Errors
    ///
    /// The daemon failed to start.
    pub fn wake(&self, idle: Duration) -> Result<bool> {
        let fresh = !daemon::running();
        if fresh {
            daemon::spawn((self.serving)(idle)?)?;
        }
        Ok(fresh)
    }

    /// A game with no run in it.
    ///
    /// A replay is a new run, so it needs one. Stopping rather than reusing,
    /// because a daemon holding a half-played run applies the recorded steps on
    /// top of it.
    ///
    /// # Errors
    ///
    /// The running daemon refused to stop, or a fresh one failed to start.
    pub fn empty(&self) -> Result<Session<V>> {
        if daemon::running() {
            self.attached().stop()?;
            while daemon::running() {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        daemon::spawn((self.serving)(DEFAULT_IDLE_TIMEOUT)?)?;
        Ok(self.attached())
    }

    /// Runs the daemon, or starts one and returns.
    ///
    /// # Errors
    ///
    /// The game directory is unset, or the socket is already taken.
    pub fn serve(&self, detach: bool, idle: Duration) -> Result<()> {
        if detach {
            daemon::spawn((self.serving)(idle)?)?;
            println!("serving at {}", daemon::socket_path().display());
            return Ok(());
        }
        Ok(daemon::serve::<V>(config()?, idle)?)
    }

    /// Reports whether a game is up, and how close it is to timing out.
    ///
    /// # Errors
    ///
    /// Nothing is holding a game.
    pub fn status(&self) -> Result<String> {
        let status = self.attached().status()?;
        let mut said = format!(
            "running  pid {}\ngame     {}\nidle     {}s of {}s",
            status.pid,
            status.game,
            status.idle.as_secs(),
            status.idle_timeout.as_secs()
        );
        if let Some(path) = status.recording {
            said.push_str(&format!("\nrecord   {}", path.display()));
        }
        Ok(said)
    }

    /// Shuts the game and the daemon down.
    ///
    /// # Errors
    ///
    /// The daemon was up and refused to stop.
    pub fn stop(&self) -> Result<String> {
        if !daemon::running() {
            return Ok("nothing running".into());
        }
        self.attached().stop()?;
        Ok("stopped".into())
    }

    /// Starts or stops writing the run down, mid-session.
    ///
    /// # Errors
    ///
    /// Nothing is holding a game, or the file cannot be written.
    pub fn record(&self, file: Option<PathBuf>) -> Result<String> {
        Ok(match self.attached().record(file, false)? {
            Recorded { path: Some(p), .. } => format!("recording to {}", p.display()),
            Recorded { steps, .. } => format!("stopped recording after {steps} step(s)"),
        })
    }
}

/// Tells the session where to write the run, and fails loudly if it cannot.
///
/// Loudly because the alternative is a run played to the end under the belief
/// that it was being kept.
///
/// # Errors
///
/// The session refused the file.
pub fn begin_recording<V: ContentVersion>(
    game: &Session<V>,
    path: PathBuf,
    append: bool,
) -> Result<()> {
    game.record(Some(path.clone()), append)?;

    let verb = match append {
        true => "still recording",
        false => "recording",
    };
    eprintln!("({verb} to {})", path.display());
    Ok(())
}

/// A recording, read from a file that has to hold a whole run.
///
/// # Errors
///
/// The file is unreadable, empty, or begins mid-run. A recording begun mid-run
/// replays into a different run, silently: the seed is unset, so every shuffle
/// differs from the first step on.
pub fn recorded<V: ContentVersion>(file: &Path) -> Result<sts2_core::Recording<V>> {
    let opened =
        std::fs::File::open(file).with_context(|| format!("opening {}", file.display()))?;
    let recording = recording::read::<V>(std::io::BufReader::new(opened))
        .with_context(|| format!("reading {}", file.display()))?;

    match recording.steps().first() {
        None => anyhow::bail!("{} has no steps in it", file.display()),
        Some(sts2_core::Step::Decision(sts2_core::Decision::Start { .. })) => Ok(recording),
        Some(_) => anyhow::bail!(
            "{} does not begin with a `start`, so it is a fragment rather than a run",
            file.display()
        ),
    }
}

/// Everything the oracle adds to what a simulator says.
///
/// The rule line and the game's log are the protocol's and not the run's. So
/// they are handed to the renderer rather than reached for by it. A front end
/// driving something else prints the same page without them.
pub fn readable<V: ContentVersion>(game: &Session<V>, applied: &Applied<V>, rng: bool) -> String {
    let mut view = Readable::new(applied).logging(game.logged());
    if let Some((seq, cmd, digest)) = game.reported().and_then(envelope) {
        view = view.in_reply(seq, cmd, digest);
    }
    if rng {
        view = view.with_rng();
    }
    format!("{view}")
}

/// The three scalars the protocol wraps a reply in.
fn envelope(reported: &sts2_headless::Response) -> Option<(u64, &str, &str)> {
    match reported {
        sts2_headless::Response::Observation {
            seq, cmd, digest, ..
        }
        | sts2_headless::Response::Pending {
            seq, cmd, digest, ..
        } => Some((*seq, cmd, digest)),
        sts2_headless::Response::Bye => None,
    }
}

/// Where the game is installed.
///
/// # Errors
///
/// `STS2_GAME_DIR` is unset.
fn config() -> Result<Config> {
    let dir =
        sts2_headless::game_dir_from_env().context("set STS2_GAME_DIR to the game's directory")?;

    Ok(Config::new(dir))
}
