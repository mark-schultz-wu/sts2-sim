//! A game held open by a daemon, driven from a process that did not launch it.
//!
//! The second implementation of [`Play`] in this crate, and the same game
//! behind it. [`Oracle`](crate::Oracle) owns a child process and dies with it.
//! This reaches one that a [`daemon`](crate::daemon) is holding, so `sts2
//! play-card 0` can be a whole process and still land in a run forty decisions
//! deep.
//!
//! One connection per command, not one held open. The crate README says why a
//! front end holds this type rather than a socket.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use sts2_core::{Answered, Construct, Decision, Play, Setup, Snapshot, Start};

use super::daemon::{
    self,
    protocol::{Answer, Request},
};
use super::error::Error;
use super::game::parse_reply;
use crate::wire::command::Command;
use crate::wire::response::Response;

/// A game somebody else is holding.
///
/// Carries no connection and no game. It is a way of addressing one. What it
/// does carry is the last reply, for the same reason [`Oracle`](crate::Oracle)
/// does. The envelope and the game's own log are oracle-specific facts a front
/// end wants, and [`sts2_core::Applied`] deliberately has no room for them.
#[derive(Debug)]
pub struct Session<V: sts2_content_wire::WireContentVersion> {
    last: Option<Response>,
    warning: Option<String>,
    build: std::marker::PhantomData<V>,
}

/// What a held session is doing, as opposed to what the run is.
#[derive(Debug, Clone)]
pub struct Status {
    pub pid: u32,
    /// The build string out of the game's greeting, or why there is not one.
    pub game: String,
    pub idle: std::time::Duration,
    pub idle_timeout: std::time::Duration,
    /// Where the run is being written, if it is.
    pub recording: Option<PathBuf>,
}

/// A recording that just started or just stopped.
#[derive(Debug, Clone)]
pub struct Recorded {
    /// Where it is being written. `None` means it stopped.
    pub path: Option<PathBuf>,
    /// How long the run is, in steps. The whole run, not this session's share.
    pub steps: usize,
}

/// By hand, not derived. A derived one demands `V: Default`, and a build is a
/// marker type nobody constructs. The derive compiles, and no caller ever
/// reaches it.
impl<V: sts2_content_wire::WireContentVersion> Default for Session<V> {
    fn default() -> Session<V> {
        Session::new()
    }
}

impl<V: sts2_content_wire::WireContentVersion> Session<V> {
    /// A way of addressing the held game. Does not connect.
    ///
    /// Deliberately infallible. The failure a caller cares about is the one
    /// their command produces. Reporting "no game is running" from a
    /// constructor means asking twice and racing in between. Every method below
    /// connects, and [`Error::NoDaemon`] is what comes back when there is
    /// nobody there.
    pub fn new() -> Session<V> {
        Session {
            last: None,
            warning: None,
            build: std::marker::PhantomData,
        }
    }

    /// What the game logged while running the most recent command.
    ///
    /// Oracle-specific, and so not on [`Play`], because a simulator has no
    /// engine log to report. Never an error on its own. The command can have
    /// half-happened, and only the caller decides whether to trust the state it
    /// produced.
    pub fn logged(&self) -> &[String] {
        self.last.as_ref().map_or(&[], Response::game_errors)
    }

    /// Something that went wrong beside the last command, which still worked.
    ///
    /// A recording that stopped is the case, and the only one so far. It is
    /// separate from [`Session::logged`] because it is the *session* failing
    /// rather than the game: the move happened and the file did not.
    pub fn warned(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    /// The most recent reply, as the protocol sent it.
    ///
    /// The envelope [`Play`] drops: the sequence number, the command name and
    /// the game's own digest. `None` before the first command.
    pub fn reported(&self) -> Option<&Response> {
        self.last.as_ref()
    }

    /// Starts, resumes or stops writing the run down.
    ///
    /// On the daemon rather than passed with each command, because the daemon
    /// is the only thing that sees the whole run. A client is one process per
    /// command. Any invocation that forgot the path leaves a gap in the file
    /// that nothing detects.
    pub fn record(&self, path: Option<PathBuf>, append: bool) -> Result<Recorded, Error> {
        match ask::<V>(&Request::Record { path, append })? {
            Answer::Recording { path, steps } => Ok(Recorded { path, steps }),
            other => Err(unexpected("record", &other)),
        }
    }

    /// What the session is doing, as opposed to what the run is.
    pub fn status(&self) -> Result<Status, Error> {
        match ask::<V>(&Request::Status)? {
            Answer::Status {
                pid,
                game,
                idle_seconds,
                idle_timeout_seconds,
                recording,
            } => Ok(Status {
                pid,
                game,
                idle: std::time::Duration::from_secs(idle_seconds),
                idle_timeout: std::time::Duration::from_secs(idle_timeout_seconds),
                recording,
            }),
            other => Err(unexpected("status", &other)),
        }
    }

    /// Ends the game and the daemon holding it.
    pub fn stop(&self) -> Result<(), Error> {
        match ask::<V>(&Request::Stop)? {
            Answer::Stopping => Ok(()),
            other => Err(unexpected("stop", &other)),
        }
    }

    /// Sends one command and reads what came back.
    fn dispatch(&mut self, command: impl Into<Command<V>>) -> Result<Answered<V>, Error> {
        let wire: Command<V> = command.into();
        let encoded =
            serde_json::to_value(&wire).map_err(|e| Error::Encode(format!("{wire:?}"), e))?;

        let (line, warning) = match ask::<V>(&Request::Game { command: encoded })? {
            Answer::Reply { line, warning } => (line, warning),
            other => return Err(unexpected("a command", &other)),
        };

        // Kept whether or not it is `Some`, so `warned()` is about the last
        // command rather than about the last command that had a warning.
        self.warning = warning;

        super::receive(parse_reply::<V>(&line)?, &mut self.last)
    }
}

impl<V: sts2_content_wire::WireContentVersion> Play<V> for Session<V> {
    type Error = Error;

    /// Nothing. The daemon is already holding a game, so there is nowhere to
    /// point this: the socket is derived from the build.
    type Host = ();

    /// Asks the daemon to start a run on the game it is holding.
    ///
    /// Whether it will is the daemon's business. It holds one game, so it is
    /// the thing that knows a run is already open. It says so, rather than
    /// starting a second one over the first.
    fn begin(_: Self::Host, start: &Start<V>) -> Result<Self, Error> {
        // The game's `start` begins a solo run.
        start.alone();
        let mut session = Session::new();
        match session.dispatch(start)? {
            Ok(_) => Ok(session),
            Err(refusal) => Err(Error::NotStarted(refusal.to_string())),
        }
    }

    fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Error> {
        self.dispatch(decision)
    }

    fn observe(&mut self) -> Result<Snapshot<V>, Error> {
        match self.dispatch(Command::Observe)? {
            Ok(applied) => Ok(applied.snapshot()),
            Err(refusal) => Err(Error::NotObserved(refusal.to_string())),
        }
    }
}

impl<V: sts2_content_wire::WireContentVersion> Construct<V> for Session<V> {
    fn setup(&mut self, step: &Setup<V>) -> Result<Answered<V>, Error> {
        self.dispatch(step)
    }
}

/// The daemon's run as a table of one seat, as the terminal plays it.
///
/// The daemon holds the run. This keeps where it last stood, so a seat's view
/// can be lent rather than asked for each time.
pub struct HeldTable<V: sts2_content_wire::WireContentVersion> {
    session: Session<V>,
    last: Snapshot<V>,
}

impl<V: sts2_content_wire::WireContentVersion> HeldTable<V> {
    /// The run the daemon is already holding.
    pub fn attached(mut session: Session<V>) -> Result<Self, Error> {
        let last = session.observe()?;
        Ok(Self { session, last })
    }

    /// A reply's answer, keeping where it left the run.
    fn settled(&mut self, answered: Answered<V>) -> sts2_core::Settled<V> {
        let (decided, snapshot) = answered?.split();
        self.last = snapshot;
        Ok(decided)
    }
}

impl<V: sts2_content_wire::WireContentVersion> sts2_core::RunTable<V> for HeldTable<V> {
    type Error = Error;
    type Host = ();
    type Seen<'a> = &'a sts2_core::Observation<V>;

    /// Asks the daemon to start a run, as [`Session`]'s `begin` does.
    fn begin(host: Self::Host, start: &Start<V>) -> Result<Self, Error> {
        Self::attached(Session::begin(host, start)?)
    }

    fn seats(&self) -> usize {
        1
    }

    fn seen(&mut self, _: sts2_core::SeatIndex) -> Result<Self::Seen<'_>, Error> {
        Ok(&self.last.observation)
    }

    fn moves(
        &mut self,
        _: sts2_core::SeatIndex,
        moves: &mut Vec<Decision<V>>,
    ) -> Result<(), Error> {
        moves.clear();
        moves.extend_from_slice(&self.last.observation.moves);
        Ok(())
    }

    fn play(
        &mut self,
        _: sts2_core::SeatIndex,
        decision: &Decision<V>,
    ) -> Result<sts2_core::Settled<V>, Error> {
        let answered = self.session.play(decision)?;
        Ok(self.settled(answered))
    }

    /// Asked of the daemon, which can idle out from under a held run.
    fn snapshot(&mut self, _: sts2_core::SeatIndex) -> Result<Snapshot<V>, Error> {
        self.last = self.session.observe()?;
        Ok(self.last.clone())
    }

    fn construct(
        &mut self,
        _: sts2_core::SeatIndex,
        setup: &Setup<V>,
    ) -> Result<sts2_core::Settled<V>, Error> {
        let answered = self.session.setup(setup)?;
        Ok(self.settled(answered))
    }
}

/// One request, one answer, one connection.
///
/// [`Answer::Failed`] is turned into an error here rather than handed on. It is
/// the daemon saying it did not do the thing, which is what an `Err` means.
/// Leaving it as a value gives every call site a second failure shape to forget
/// about.
fn ask<V: sts2_content_wire::WireContentVersion>(request: &Request) -> Result<Answer, Error> {
    let path = daemon::socket_path(&V::VERSION);
    let stream = UnixStream::connect(&path).map_err(|_| Error::NoDaemon(path))?;

    let mut writer = stream
        .try_clone()
        .map_err(|e| Error::Io("opening a connection to the daemon", e))?;
    let mut reader = BufReader::new(stream);

    let encoded =
        serde_json::to_string(request).map_err(|e| Error::Encode(format!("{request:?}"), e))?;
    writeln!(writer, "{encoded}")
        .and_then(|()| writer.flush())
        .map_err(|e| Error::Io("sending a request to the daemon", e))?;

    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => {
            return Err(Error::Unanswered);
        }
        Ok(_) => {}
        Err(e) => return Err(Error::Io("reading the daemon's answer", e)),
    }

    let answer: Answer =
        serde_json::from_str(line.trim()).map_err(|e| Error::Decode(line.trim().to_string(), e))?;

    match answer {
        Answer::Failed { message } => Err(Error::Daemon(message)),
        answer => Ok(answer),
    }
}

fn unexpected(asked: &'static str, got: &Answer) -> Error {
    Error::Unexpected {
        asked,
        got: format!("{got:?}"),
    }
}
