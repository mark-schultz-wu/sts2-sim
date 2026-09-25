//! The process that holds the game open.
//!
//! One game, one socket, one client at a time. Requests are served in the order
//! they arrive, because the game is a single state machine. Two commands in
//! flight are two people playing one run.
//!
//! # Why this is here and not in a front end
//!
//! Because it is a fact about *this* implementation and not about playing.
//! The game boots in seconds and holds every bit of run state, so a command
//! cannot be a whole process. That is the oracle's problem. A simulator is a
//! struct in the caller's own address space. Asking a front end which of the
//! two it is talking to puts a socket in the vocabulary. That vocabulary exists
//! to have no socket in it.
//!
//! So a caller holds a [`Session`](crate::Session) and drives it through
//! [`Play`](sts2_core::Play), exactly as it drives an
//! [`Oracle`](crate::Oracle) or a simulator. It never learns that any of this
//! is here.

pub(crate) mod protocol;

use protocol::{Answer, Request};

use super::config::Config;
use super::error::Error;
use super::game::Oracle;
use crate::recording;
use crate::wire::command::Command;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use sts2_core::Step;
use sts2_core::Version;

type Result<T> = std::result::Result<T, Error>;

/// What stopped the daemon: a path that did not open, or a socket already
/// bound. It reaches a client as [`Error::Daemon`], worded, over the socket.
#[derive(Debug, thiserror::Error)]
pub enum Failure {
    #[error("{what} {}: {source}", path.display())]
    File {
        what: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("writing the build stamp: {0}")]
    Stamp(std::io::Error),
    #[error("{} is empty; there is no recording to carry on", .0.display())]
    Empty(PathBuf),
    #[error("this build cannot replay {command}: {source}")]
    Unreplayable {
        command: serde_json::Value,
        source: serde_json::Error,
    },
    #[error("{0} is a command but not a step in a run")]
    NotAStep(serde_json::Value),
    #[error("a game is already running at {}; stop it first", .0.display())]
    Running(PathBuf),
    #[error("the listener has to be pollable so the timeout can fire: {0}")]
    Blocking(std::io::Error),
    #[error("spawning the daemon: {0}")]
    Spawn(std::io::Error),
    #[error("the daemon (pid {pid}) did not bind {} within ten seconds", socket.display())]
    Unbound { pid: u32, socket: PathBuf },
}

/// A file that failed the daemon, by what it was doing to it.
fn file(what: &'static str, path: &Path) -> impl FnOnce(std::io::Error) -> Error {
    move |source| {
        Failure::File {
            what,
            path: path.to_path_buf(),
            source,
        }
        .into()
    }
}

/// How long the daemon waits before deciding nobody is coming back.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// Where the socket lives.
///
/// `XDG_RUNTIME_DIR` when there is one, because it is per-user, cleaned up at
/// logout, and not world-writable. `/tmp` otherwise, with the uid in the name so
/// two users on one machine do not collide.
///
/// One socket for each build. The daemon relays commands the client encoded
/// against its own tables, so reaching the wrong daemon reads a game through
/// the wrong ids.
pub fn socket_path(version: &Version) -> PathBuf {
    let name = format!("sts2-{}.sock", tag(version.name));
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) => PathBuf::from(dir).join(name),
        None => PathBuf::from(format!("/tmp/sts2-{}-{}", uid(), name)),
    }
}

/// A build's name as one path component. `v0.107.1` is `v0_107_1`.
fn tag(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// The user, for keeping two people's sockets apart under `/tmp`.
///
/// Read from the environment rather than pulled in with a `libc` dependency for
/// one number. It is a name, not a permission: the socket's own mode is what
/// stops anyone else connecting, and this only avoids a collision.
fn uid() -> String {
    std::env::var("UID")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "shared".into())
}

/// Whether a daemon is listening.
///
/// Connecting rather than checking for the file. A socket left behind by a
/// daemon that died is a file with nobody on the other end.
pub fn running(version: &Version) -> bool {
    alive(&socket_path(version))
}

/// Whether anyone answers on one socket.
///
/// By path rather than by build. Two callers already hold a path: the bind that
/// lost a race, and the wait for a daemon just spawned.
fn alive(path: &Path) -> bool {
    UnixStream::connect(path).is_ok()
}

/// A run being written down as it is played.
///
/// Held by the daemon, because the daemon is the only thing that sees the whole
/// run. A client is one process per command. Each command is written and
/// flushed as it goes. A session that is killed or times out then leaves a file
/// complete up to its last decision.
struct Tape {
    path: PathBuf,
    file: File,
    steps: usize,
}

impl Tape {
    /// Begins a recording, replacing whatever was at the path.
    fn open<V: sts2_content_wire::WireContentVersion>(path: &Path) -> Result<Self> {
        let mut created = File::create(path).map_err(file("creating", path))?;

        recording::open::<V>(&mut created).map_err(Failure::Stamp)?;
        created.flush().map_err(file("writing", path))?;

        Ok(Self {
            path: path.to_path_buf(),
            file: created,
            steps: 0,
        })
    }

    /// Carries on a recording that already exists.
    ///
    /// What a replay wants. The file already holds the steps being replayed.
    /// Rewriting it means destroying the run to reproduce it. A replay that
    /// fails halfway then leaves the file somebody opened to investigate
    /// truncated at the failure.
    fn resume(path: &Path) -> Result<Self> {
        let existing = std::fs::read_to_string(path).map_err(file("reading", path))?;

        // Everything but the stamp. Counted rather than assumed zero, so
        // "stopped recording after N steps" is the run's length rather than
        // this session's share of it — and checked rather than subtracted from,
        // because a request arrives over a socket and an empty file would take
        // the daemon down with an underflow.
        let lines = existing.lines().filter(|l| !l.trim().is_empty()).count();
        let steps = lines
            .checked_sub(1)
            .ok_or_else(|| Failure::Empty(path.to_path_buf()))?;

        let appending = File::options()
            .append(true)
            .open(path)
            .map_err(file("appending to", path))?;

        Ok(Self {
            path: path.to_path_buf(),
            file: appending,
            steps,
        })
    }

    /// Records one command, or gives up recording rather than writing a gap.
    ///
    /// The daemon relays commands untyped, but writes them typed. A line this
    /// build cannot read back is a line that will not replay.
    ///
    /// A failure ends the recording. A file missing a step in the middle
    /// replays as a different run, with nothing about it saying so.
    fn write<V: sts2_content_wire::WireContentVersion>(
        &mut self,
        command: &serde_json::Value,
    ) -> Result<()> {
        let wire: Command<V> =
            serde_json::from_value(command.clone()).map_err(|source| Failure::Unreplayable {
                command: command.clone(),
                source,
            })?;
        let step: Step<V> = wire
            .into_step()
            .ok_or_else(|| Failure::NotAStep(command.clone()))?;

        recording::append(&mut self.file, &step)
            .and_then(|()| self.file.flush())
            .map_err(file("writing", &self.path))?;
        self.steps += 1;
        Ok(())
    }
}

/// Whether a command is a decision, and so part of the run.
///
/// `observe` and `quit` are not. Observing changes nothing, so replaying it
/// only slows a replay down. Quitting ends the session rather than the run. A
/// recording that ends by shutting the game down cannot be replayed past its
/// own last step.
fn is_decision(command: &serde_json::Value) -> bool {
    !matches!(
        command.get("cmd").and_then(serde_json::Value::as_str),
        Some("observe") | Some("quit")
    )
}

/// Runs until told to stop, or until nobody has asked for anything in a while.
///
/// The timeout is not a nicety. A client is one short-lived process per command,
/// so nothing observes the daemon's health and nothing tidies up after a client
/// that crashed. Without it, a forgotten game sits holding memory until the
/// machine reboots.
pub fn serve<V: sts2_content_wire::WireContentVersion>(
    config: Config,
    idle_timeout: Duration,
) -> Result<()> {
    let path = socket_path(&V::VERSION);

    // Bind first, and only clear the path once nobody has answered on it.
    //
    // Unlinking unconditionally looked safe and is not: nothing stops a second
    // daemon starting — `sts2 daemon serve` is documented and takes no lock — and the
    // second would unlink the first's socket, orphaning a game. Worse, the
    // orphan unlinks the *live* socket when it idles out, after which every
    // command reports no game while a daemon still holds the run.
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            if alive(&path) {
                return Err(Failure::Running(path).into());
            }
            // Nobody answered, so this is a dead daemon's leavings.
            std::fs::remove_file(&path).map_err(file("clearing a stale", &path))?;
            UnixListener::bind(&path).map_err(file("binding", &path))?
        }
        Err(e) => return Err(file("binding", &path)(e)),
    };
    listener.set_nonblocking(true).map_err(Failure::Blocking)?;

    let mut game: Option<Oracle<V>> = None;
    // Whether a run is open on `game`. See `relay`.
    let mut started = false;
    let mut tape: Option<Tape> = None;
    let mut last = Instant::now();

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                last = Instant::now();
                if !serve_one::<V>(
                    stream,
                    &config,
                    &mut game,
                    &mut started,
                    &mut tape,
                    idle_timeout,
                    last,
                )? {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if last.elapsed() >= idle_timeout {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(Error::Io("accepting a client", e)),
        }
    }

    let _ = std::fs::remove_file(&path);
    Ok(())
}

/// Handles one connection. Returns `false` when the daemon is to stop.
fn serve_one<V: sts2_content_wire::WireContentVersion>(
    stream: UnixStream,
    config: &Config,
    game: &mut Option<Oracle<V>>,
    started: &mut bool,
    tape: &mut Option<Tape>,
    idle_timeout: Duration,
    last: Instant,
) -> Result<bool> {
    stream
        .set_nonblocking(false)
        .map_err(|e| Error::Io("taking a client connection", e))?;
    let writer = stream
        .try_clone()
        .map_err(|e| Error::Io("taking a client connection", e))?;
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => return Ok(true),
        Ok(_) => {}
        // A client that died mid-write is not a reason to end the run.
        Err(e) => {
            return answer_with(writer, &failed(format!("reading a request: {e}"))).map(|()| true);
        }
    }

    // Not `?`. Anything unreadable on this socket — a client from another build
    // sending a request kind this one does not know, something probing the port
    // — would otherwise end the daemon and take a run dozens of decisions deep
    // with it. Every other failure here already answers and carries on.
    let request: Request = match serde_json::from_str(line.trim()) {
        Ok(request) => request,
        Err(e) => {
            let message = format!("could not read a request: {e}; line was: {}", line.trim());
            return answer_with(writer, &failed(message)).map(|()| true);
        }
    };

    let (answer, keep_going) = match request {
        Request::Stop => (Answer::Stopping, false),
        Request::Status => (
            Answer::Status {
                pid: std::process::id(),
                game: match game {
                    Some(g) => g.greeting().game.clone(),
                    None => "not launched".into(),
                },
                idle_seconds: last.elapsed().as_secs(),
                idle_timeout_seconds: idle_timeout.as_secs(),
                recording: tape.as_ref().map(|t| t.path.clone()),
            },
            true,
        ),
        Request::Record { path, append } => (record::<V>(tape, path, append), true),
        Request::Game { command } => (relay::<V>(config, game, started, tape, command), true),
    };

    answer_with(writer, &answer)?;

    Ok(keep_going)
}

fn failed(message: String) -> Answer {
    Answer::Failed { message }
}

fn answer_with(mut writer: UnixStream, answer: &Answer) -> Result<()> {
    let encoded =
        serde_json::to_string(answer).map_err(|e| Error::Encode(format!("{answer:?}"), e))?;
    writeln!(writer, "{encoded}")
        .and_then(|()| writer.flush())
        .map_err(|e| Error::Io("answering a client", e))
}

/// Starts, resumes or stops writing the run down.
fn record<V: sts2_content_wire::WireContentVersion>(
    tape: &mut Option<Tape>,
    path: Option<PathBuf>,
    append: bool,
) -> Answer {
    let Some(path) = path else {
        // "stopped after 0 steps" was what this used to say when there was
        // nothing to stop, which reads as a run that recorded nothing rather
        // than as a run that was never being recorded.
        return match tape.take() {
            Some(t) => Answer::Recording {
                path: None,
                steps: t.steps,
            },
            None => Answer::Failed {
                message: "not recording".into(),
            },
        };
    };

    let opened = match append {
        true => Tape::resume(&path),
        false => Tape::open::<V>(&path),
    };

    match opened {
        Ok(fresh) => {
            let steps = fresh.steps;
            *tape = Some(fresh);
            Answer::Recording {
                path: Some(path),
                steps,
            }
        }
        Err(e) => Answer::Failed {
            message: format!("{e:#}"),
        },
    }
}

/// Whether a command is the one that starts a run.
///
/// Read off the wire rather than typed. The daemon relays a `Value`, and the
/// typing happens once at the client's edge. This is the one command it has to
/// know, because it is the one that must not happen twice.
fn starts_a_run(command: &serde_json::Value) -> bool {
    command.get("cmd").and_then(|cmd| cmd.as_str()) == Some("start")
}

/// Sends one command to the game, launching it on the first one.
///
/// Launching lazily means `sts2 run start` is the only command that pays the
/// boot cost. A daemon spawned in the background is then useful the moment it
/// binds, rather than twenty seconds later.
fn relay<V: sts2_content_wire::WireContentVersion>(
    config: &Config,
    game: &mut Option<Oracle<V>>,
    started: &mut bool,
    tape: &mut Option<Tape>,
    command: serde_json::Value,
) -> Answer {
    // One run per held game. A second `start` replaces a run this daemon holds
    // for somebody, leaving its clients a decision deep in a run that is gone.
    // The trait says nothing about this: it is a fact about holding one game.
    if starts_a_run(&command) && *started {
        return Answer::Failed {
            message: "a run is already open on this game; stop it first".into(),
        };
    }

    if game.is_none() {
        match Oracle::<V>::launch(config.clone()) {
            Ok(fresh) => {
                *game = Some(fresh);
                *started = false;
            }
            Err(e) => {
                return Answer::Failed {
                    message: format!("launching the game: {e}"),
                };
            }
        }
    }

    let held = game.as_mut().expect("just launched");
    let encoded = match serde_json::to_string(&command) {
        Ok(encoded) => encoded,
        Err(e) => {
            return Answer::Failed {
                message: format!("encoding the command: {e}"),
            };
        }
    };

    match held.send_line(&encoded) {
        Ok(line) => {
            // A refused start opened nothing, so the game is still free for the
            // next one to try.
            if starts_a_run(&command) && !refused(&line) {
                *started = true;
            }
            // Recorded after the game accepted it, and only if it did: a command
            // the game refused did not happen, and replaying it would either be
            // refused again — noise — or, worse, be legal in the replayed run
            // and take it somewhere the original never went.
            let mut warning = None;
            if let Some(recording) = tape.as_mut()
                && is_decision(&command)
                && !refused(&line)
                && let Err(e) = recording.write::<V>(&command)
            {
                // The recording stops — a file missing a step in the middle
                // replays as a different run and says nothing about it. The
                // reply does not: the command was accepted and applied, and
                // turning that into a failure would leave the caller believing
                // a move it already made did not happen.
                *tape = None;
                warning = Some(format!("the recording stopped: {e:#}"));
            }
            Answer::Reply { line, warning }
        }
        Err(e) => {
            // A transport failure means the game is gone or out of step, and a
            // half-dead session answers worse than no session.
            *game = None;
            Answer::Failed {
                message: format!("{e}"),
            }
        }
    }
}

/// Whether the game's reply says it did nothing.
///
/// By the wire's own rule, rather than by parsing the reply into a typed
/// `Response`. The daemon is a relay and does not otherwise read what it passes
/// along, and `"type": "error"` is the whole of the protocol's answer here.
fn refused(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|v| {
            v.get("type")
                .and_then(serde_json::Value::as_str)
                .map(|t| t == "error")
        })
        .unwrap_or(false)
}

/// Starts a daemon with the caller's own command, and waits for it to bind.
///
/// The command is the caller's, because *which binary serves* is. This crate
/// owns the socket and the loop, and a front end owns the fact that it has a
/// `serve` subcommand to re-enter itself through. Whatever is handed in must
/// end up calling [`serve`].
pub fn spawn(mut command: std::process::Command, socket: &Path) -> Result<()> {
    let child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(Failure::Spawn)?;

    // Waiting for the socket rather than for the process: a daemon that has not
    // bound yet is one a client would fail to reach.
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if alive(socket) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    Err(Failure::Unbound {
        pid: child.id(),
        socket: socket.to_path_buf(),
    }
    .into())
}
