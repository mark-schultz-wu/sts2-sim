//! A client for the `sts2_headless` mod.
//!
//! The game is the oracle the simulator gets checked against: the real thing,
//! run headless, driven one decision at a time. This crate owns the transport
//! and nothing above it: launching the game, the handshake, and one typed
//! request/response pair per call. What the state *means* is deliberately left
//! alone — see [`response`].
//!
//! [PROTOCOL.md](../../../mod/sts2_headless/PROTOCOL.md) is the contract this
//! implements.
//!
//! ```no_run
//! use sts2_headless::{Command, Config, Game, Response};
//!
//! let mut game = Game::launch(Config::new("/path/to/Slay the Spire 2"))?;
//! let reply = game.send(&Command::Start {
//!     seed: "ABCDEF".into(),
//!     character: "IRONCLAD".into(),
//!     ascension: 0,
//!     modifiers: vec![],
//! })?;
//!
//! // The outer `?` above covered the transport. This is the game's answer.
//! match reply {
//!     Ok(Response::Observation { digest, .. }) => println!("agreed at {digest}"),
//!     Ok(other) => println!("{other}"),
//!     Err(refusal) => println!("{refusal}"),
//! }
//! # Ok::<(), sts2_headless::Error>(())
//! ```

mod command;
mod display;
mod response;

pub use command::{Answer, Command, TakeReward};
pub use response::{CardOption, Question, Refusal, Response, RewardDetail, RewardKind};

/// What one command produced.
///
/// The inner `Err` is the game declining the command; the session is fine and
/// the next command will work. Contrast [`Error`], the outer failure, which
/// means the transport or the game process is in trouble and nothing further
/// will work. Two layers because they are genuinely two questions: *did this
/// command happen*, and *can I send another one*.
pub type Reply = Result<Response, Refusal>;

use serde::Deserialize;
use serde_json::Value;
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command as Proc, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The protocol version this crate speaks. A mismatch is refused at the
/// handshake rather than discovered field by field later.
pub const PROTOCOL_VERSION: u32 = 1;

const BINARY: &str = "SlayTheSpire2";

/// How to launch the game.
#[derive(Debug, Clone)]
pub struct Config {
    /// The directory holding the `SlayTheSpire2` binary. Also the working
    /// directory, which the game expects.
    pub game_dir: PathBuf,
    /// Let the game's own output through to this process's stdout and stderr.
    ///
    /// Off by default, and that matters: the engine writes ~140 unprefixed lines
    /// during startup, which would scribble over a TUI. Turn it on to debug a
    /// launch that is failing.
    pub inherit_game_output: bool,
    /// How long to wait for the game to start and connect back.
    pub startup_timeout: Duration,
    /// How long to wait for a reply to one command.
    pub command_timeout: Duration,
}

impl Config {
    pub fn new(game_dir: impl Into<PathBuf>) -> Self {
        Self {
            game_dir: game_dir.into(),
            inherit_game_output: false,
            // Measured at ~3s from spawn to a connected game — the ~10s the
            // mod README quotes is startup to the main menu, which serve mode
            // never reaches. The slack is for a loaded machine, not for a game
            // that is never going to arrive.
            startup_timeout: Duration::from_secs(60),
            command_timeout: Duration::from_secs(30),
        }
    }

    /// Let the game's output through. See [`Config::inherit_game_output`].
    pub fn inherit_game_output(mut self, yes: bool) -> Self {
        self.inherit_game_output = yes;
        self
    }
}

/// A running game, and the process behind it.
///
/// Dropping this closes the connection, which the game treats as a normal
/// shutdown, and then reaps the process — so a client that dies does not leave a
/// game behind.
pub struct Game {
    child: Child,
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    greeting: Greeting,
    command_timeout: Duration,
}

/// What the game said about itself when it connected.
///
/// Deserialized directly rather than through [`Response`], because the greeting
/// arrives once before any request and is never a reply to one — putting it in
/// the response enum would oblige every caller to match a case none can reach.
#[derive(Debug, Clone, Deserialize)]
pub struct Greeting {
    pub protocol: u32,
    pub game: String,
    #[serde(rename = "assemblyHash")]
    pub assembly_hash: i64,
    /// Every command this game accepts.
    pub commands: Vec<String>,
}

impl Game {
    /// Launches the game and waits for it to connect back.
    ///
    /// The game dials out: this binds an ephemeral port first and passes it to
    /// the game, so there is no port-allocation race and no connect-retry loop.
    pub fn launch(config: Config) -> Result<Self, Error> {
        let binary = config.game_dir.join(BINARY);
        if !binary.exists() {
            return Err(Error::GameNotFound(binary));
        }

        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .map_err(|e| Error::Io("binding a port for the game to dial", e))?;
        let port = listener
            .local_addr()
            .map_err(|e| Error::Io("reading the bound port", e))?
            .port();
        let token = fresh_token();

        let stdio = || {
            if config.inherit_game_output {
                Stdio::inherit()
            } else {
                Stdio::null()
            }
        };

        let mut proc = Proc::new(&binary);
        proc.current_dir(&config.game_dir)
            .args(["--headless", "--force-steam=off"])
            .env("STS2_HEADLESS_MODE", "serve")
            .env("STS2_HEADLESS_PORT", port.to_string())
            .env("STS2_HEADLESS_TOKEN", &token)
            // Always. The digest-only mode saves bandwidth and log size, not
            // time -- H7 measures the two within 5% of each other, because a
            // decision costs frame-boundary latency rather than serialisation.
            // Paying ~200KB/s to make `state` unconditional is the better trade,
            // and it is what lets Observation and Pending carry a Value rather
            // than an Option nobody can reason about.
            .env("STS2_HEADLESS_OBS", "full")
            // Harmony patching does not work without this, and fails in a way
            // that looks like something else. MonoMod writes a native helper to
            // /tmp with no DT_NEEDED entries at all, yet importing _Unwind_*
            // from libgcc; SlayTheSpire2 does not link libgcc, so the dlopen
            // fails with "undefined symbol: _Unwind_RaiseException" and every
            // patch throws. Preloading puts those symbols in the global
            // namespace before the helper is opened.
            .env("LD_PRELOAD", preload())
            .stdin(Stdio::null())
            .stdout(stdio())
            .stderr(stdio());

        let mut child = proc
            .spawn()
            .map_err(|e| Error::Io("launching the game", e))?;

        // From here on the child is ours: every failure path has to reap it, or
        // a failed launch leaves a headless game running forever.
        match Self::connect(&listener, &mut child, &token, &config) {
            Ok((writer, reader, greeting)) => Ok(Self {
                child,
                writer,
                reader,
                greeting,
                command_timeout: config.command_timeout,
            }),
            Err(e) => {
                reap(&mut child);
                Err(e)
            }
        }
    }

    /// Accepts the connection, checks the token, and reads the greeting.
    fn connect(
        listener: &TcpListener,
        child: &mut Child,
        token: &str,
        config: &Config,
    ) -> Result<(TcpStream, BufReader<TcpStream>, Greeting), Error> {
        let stream = accept_before(listener, child, config.startup_timeout)?;
        stream
            .set_read_timeout(Some(config.startup_timeout))
            .map_err(|e| Error::Io("setting the handshake deadline", e))?;

        let writer = stream
            .try_clone()
            .map_err(|e| Error::Io("splitting the connection", e))?;
        let mut reader = BufReader::new(stream);

        // A loopback listener accepts from any local process, so the token is
        // what turns "something else connected first" from a baffling parse
        // error into a stated one. It is a confusion guard, not a secret.
        let presented = read_line(&mut reader)?;
        if presented != token {
            return Err(Error::WrongPeer);
        }

        let line = read_line(&mut reader)?;
        let greeting = parse_greeting(&line)?;

        if greeting.protocol != PROTOCOL_VERSION {
            return Err(Error::ProtocolMismatch {
                expected: PROTOCOL_VERSION,
                found: greeting.protocol,
            });
        }

        // The startup deadline was for a game that had not booted yet; commands
        // get the shorter one.
        reader
            .get_ref()
            .set_read_timeout(Some(config.command_timeout))
            .map_err(|e| Error::Io("setting the command deadline", e))?;

        Ok((writer, reader, greeting))
    }

    /// What the game said about itself: protocol version, game build, and
    /// every command it accepts.
    pub fn greeting(&self) -> &Greeting {
        &self.greeting
    }

    /// Sends one command and returns what it produced.
    ///
    /// The outer `Result` is whether the exchange happened at all; the inner one
    /// is whether the game accepted the command. See [`Reply`].
    pub fn send(&mut self, command: &Command) -> Result<Reply, Error> {
        let line = serde_json::to_string(command).map_err(|e| Error::Encode(command.clone(), e))?;

        self.writer
            .write_all(line.as_bytes())
            .and_then(|()| self.writer.write_all(b"\n"))
            .and_then(|()| self.writer.flush())
            .map_err(|e| Error::Io("sending a command", e))?;

        let line = read_line(&mut self.reader)?;
        parse_reply(&line)
    }

    /// How long a single command may take before [`Error::Timeout`].
    pub fn command_timeout(&self) -> Duration {
        self.command_timeout
    }

    /// The game process's id, for diagnostics and for checking it has gone.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        // Closing the socket is the documented shutdown: the game sees EOF and
        // exits. Shutting down explicitly rather than relying on field drop
        // order, so the child is already on its way out before we wait for it.
        let _ = self.writer.shutdown(std::net::Shutdown::Both);
        reap(&mut self.child);
    }
}

impl fmt::Debug for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Game")
            .field("pid", &self.child.id())
            .field("game", &self.greeting.game)
            .field("protocol", &self.greeting.protocol)
            .finish_non_exhaustive()
    }
}

/// Waits for the game to dial in, giving up if the deadline passes or the
/// process dies first.
///
/// Polling rather than a blocking accept, because a game that crashes during
/// startup would otherwise hang the caller until the deadline for no reason.
fn accept_before(
    listener: &TcpListener,
    child: &mut Child,
    timeout: Duration,
) -> Result<TcpStream, Error> {
    listener
        .set_nonblocking(true)
        .map_err(|e| Error::Io("waiting for the game to connect", e))?;

    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .map_err(|e| Error::Io("returning the connection to blocking mode", e))?;
                return Ok(stream);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(Error::Io("waiting for the game to connect", e)),
        }

        if let Ok(Some(status)) = child.try_wait() {
            return Err(match status.code() {
                Some(code) => Error::GameExited(code),
                // No exit code means a signal killed it.
                None => Error::GameKilled,
            });
        }

        if Instant::now() >= deadline {
            return Err(Error::Timeout("waiting for the game to connect"));
        }

        std::thread::sleep(Duration::from_millis(25));
    }
}

fn read_line(reader: &mut BufReader<TcpStream>) -> Result<String, Error> {
    let mut line = String::new();
    match reader.read_line(&mut line) {
        // A clean EOF here means the game went away mid-conversation, which is
        // never something the caller asked for.
        Ok(0) => Err(Error::GameGone),
        Ok(_) => Ok(line.trim_end().to_string()),
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) =>
        {
            Err(Error::Timeout("waiting for a reply"))
        }
        Err(e) => Err(Error::Io("reading a reply", e)),
    }
}

/// Splits a reply into "the game refused" and "the game answered".
///
/// The discriminator is read before deserializing, so a refusal is routed by
/// what it says it is rather than by which struct happens to accept it.
fn parse_reply(line: &str) -> Result<Reply, Error> {
    let decode = |e| Error::Decode(line.to_string(), e);
    let value: Value = serde_json::from_str(line).map_err(decode)?;

    if value.get("type").and_then(Value::as_str) == Some("error") {
        return Ok(Err(serde_json::from_value(value).map_err(decode)?));
    }
    Ok(Ok(serde_json::from_value(value).map_err(decode)?))
}

/// Reads the one message that is not a [`Response`].
///
/// Checked in two steps so a wrong message names itself. Deserializing straight
/// into [`Greeting`] would report a missing `protocol` field when what actually
/// happened is that the game said `bye`.
fn parse_greeting(line: &str) -> Result<Greeting, Error> {
    let value: Value =
        serde_json::from_str(line).map_err(|e| Error::Decode(line.to_string(), e))?;
    match value.get("type").and_then(Value::as_str) {
        Some("hello") => {
            serde_json::from_value(value).map_err(|e| Error::Decode(line.to_string(), e))
        }
        Some(other) => Err(Error::NoGreeting(other.to_string())),
        None => Err(Error::NoGreeting("untyped".to_string())),
    }
}

/// Ends the game process, politely if it will go on its own.
fn reap(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            // Out of patience, or the wait itself failed: stop asking.
            _ => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();
}

/// `libgcc_s.so.1`, ahead of anything the caller already asked for.
///
/// Prepended rather than assigned, so a caller preloading something of their own
/// keeps it. Linux-only, which this project is.
fn preload() -> String {
    match std::env::var("LD_PRELOAD") {
        Ok(existing) if !existing.trim().is_empty() => format!("libgcc_s.so.1:{existing}"),
        _ => "libgcc_s.so.1".to_string(),
    }
}

/// A per-session token, to notice a stray local connection.
///
/// Not a secret and not trying to be: it distinguishes our game from something
/// else that happened to dial the port, which is the whole job. Anything on this
/// machine can read the environment it is passed in.
fn fresh_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}-{:x}", std::process::id(), nanos)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No `SlayTheSpire2` where the config said it would be.
    #[error("no game binary at {0}")]
    GameNotFound(PathBuf),

    /// The game exited before connecting back. Set
    /// [`Config::inherit_game_output`] to see why.
    #[error("the game exited with status {0} before connecting")]
    GameExited(i32),

    /// The game was killed before connecting back.
    #[error("the game was killed before connecting")]
    GameKilled,

    /// Something connected to our port, but it was not our game.
    #[error("something other than the game connected to our port")]
    WrongPeer,

    /// The game's first message was not a greeting.
    #[error("expected a greeting, got a '{0}' message")]
    NoGreeting(String),

    /// The game speaks a protocol this crate does not.
    #[error("the game speaks protocol v{found}, this client speaks v{expected}")]
    ProtocolMismatch { expected: u32, found: u32 },

    /// The connection closed mid-conversation.
    #[error("the game closed the connection")]
    GameGone,

    /// A deadline passed. The session is not usable afterwards.
    #[error("timed out {0}")]
    Timeout(&'static str),

    #[error("{0}: {1}")]
    Io(&'static str, #[source] std::io::Error),

    #[error("could not encode {0:?}: {1}")]
    Encode(Command, #[source] serde_json::Error),

    /// A line arrived that this crate could not read as a response.
    #[error("could not read a response: {1}; line was: {0}")]
    Decode(String, #[source] serde_json::Error),
}

/// Where the game is, if the environment says.
///
/// `STS2_GAME_DIR` is the same variable the justfile and `deploy.sh` read, so a
/// shell set up to build the mod is already set up to launch it.
pub fn game_dir_from_env() -> Option<PathBuf> {
    std::env::var_os("STS2_GAME_DIR").map(PathBuf::from)
}

#[cfg(test)]
mod handshake {
    use super::*;

    const HELLO: &str = r#"{"type":"hello","protocol":1,"game":"v0.107.1",
        "assemblyHash":1692500715,"commands":["start","quit"]}"#;

    /// A stand-in for the game: a process that stays alive long enough to be
    /// reaped, so [`accept_before`] does not mistake it for a crashed launch.
    fn idle_child() -> Child {
        Proc::new("sleep")
            .arg("60")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("sleep should spawn")
    }

    fn quick(timeout: Duration) -> Config {
        let mut config = Config::new(".");
        config.startup_timeout = timeout;
        config.command_timeout = timeout;
        config
    }

    /// Drives [`Game::connect`] against a peer speaking `lines`.
    ///
    /// The peer connects *before* the accept, sitting in the listener's backlog,
    /// which keeps this single-threaded — and keeps its stream alive for the
    /// whole handshake without a sleep to hold it open.
    fn against(token: &str, lines: &[&str]) -> Result<Greeting, Error> {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();

        let mut peer = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        for line in lines {
            writeln!(peer, "{}", line.replace('\n', "")).unwrap();
        }
        peer.flush().unwrap();

        let mut child = idle_child();
        let result = Game::connect(&listener, &mut child, token, &quick(Duration::from_secs(5)))
            .map(|(_, _, hello)| hello);
        reap(&mut child);
        result
    }

    #[test]
    fn a_matching_token_and_greeting_get_through() {
        let greeting = against("tok", &["tok", HELLO]).expect("handshake should succeed");
        assert_eq!(greeting.protocol, PROTOCOL_VERSION);
        assert_eq!(greeting.game, "v0.107.1");
        assert_eq!(greeting.assembly_hash, 1_692_500_715);
        assert_eq!(greeting.commands, ["start", "quit"]);
    }

    #[test]
    fn a_stranger_on_our_port_is_refused() {
        let err = against("tok", &["not-our-token", HELLO]).expect_err("should refuse");
        assert!(matches!(err, Error::WrongPeer), "got {err:?}");
    }

    #[test]
    fn the_first_message_has_to_be_a_greeting() {
        let err = against("tok", &["tok", r#"{"type":"bye"}"#]).expect_err("should refuse");
        assert!(matches!(err, Error::NoGreeting(_)), "got {err:?}");
    }

    #[test]
    fn a_protocol_we_do_not_speak_is_refused_up_front() {
        let future = r#"{"type":"hello","protocol":2,"game":"v9","assemblyHash":1,"commands":[]}"#;
        let err = against("tok", &["tok", future]).expect_err("should refuse");
        assert!(
            matches!(
                err,
                Error::ProtocolMismatch {
                    expected: 1,
                    found: 2
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn a_greeting_that_is_not_json_names_the_line() {
        let err = against("tok", &["tok", "not json at all"]).expect_err("should refuse");
        let Error::Decode(line, _) = err else {
            panic!("expected a decode error");
        };
        assert_eq!(line, "not json at all");
    }

    /// The peer hanging up mid-handshake must not read as a successful anything.
    #[test]
    fn a_peer_that_says_nothing_is_gone_not_silent() {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();

        let peer = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        drop(peer);

        let mut child = idle_child();
        let err = Game::connect(&listener, &mut child, "tok", &quick(Duration::from_secs(5)))
            .map(|_| ())
            .expect_err("should fail");
        reap(&mut child);
        assert!(matches!(err, Error::GameGone), "got {err:?}");
    }

    /// A game that dies during startup should say so immediately rather than
    /// making the caller wait out the whole deadline.
    #[test]
    fn a_game_that_exits_is_noticed_before_the_deadline() {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut child = Proc::new("false")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("false should spawn");

        let began = Instant::now();
        let err = accept_before(&listener, &mut child, Duration::from_secs(30))
            .map(|_| ())
            .expect_err("should fail");

        assert!(matches!(err, Error::GameExited(_)), "got {err:?}");
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "took {:?}, so it waited out the deadline instead of watching the child",
            began.elapsed()
        );
    }

    #[test]
    fn nobody_connecting_times_out() {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut child = idle_child();

        let err = accept_before(&listener, &mut child, Duration::from_millis(150))
            .map(|_| ())
            .expect_err("should fail");
        reap(&mut child);
        assert!(matches!(err, Error::Timeout(_)), "got {err:?}");
    }

    #[test]
    fn a_missing_game_is_caught_before_anything_is_spawned() {
        let config = Config::new("/nonexistent/game/dir");
        let err = Game::launch(config).map(|_| ()).expect_err("should fail");
        assert!(matches!(err, Error::GameNotFound(_)), "got {err:?}");
    }

    /// These strings are what a caller sees when something goes wrong, and
    /// `thiserror` generates them from attributes rather than a match, so it is
    /// worth pinning that they still read as sentences and still chain.
    #[test]
    fn errors_say_something_useful() {
        use std::error::Error as _;

        let missing = Error::GameNotFound("/nope/SlayTheSpire2".into());
        assert_eq!(missing.to_string(), "no game binary at /nope/SlayTheSpire2");
        assert!(missing.source().is_none());

        let mismatch = Error::ProtocolMismatch {
            expected: 1,
            found: 2,
        };
        assert_eq!(
            mismatch.to_string(),
            "the game speaks protocol v2, this client speaks v1"
        );

        // The cause has to survive, or `{:#}` and anyhow's chain lose the
        // reason the io failed at all.
        let io = Error::Io("launching the game", std::io::Error::other("boom"));
        assert_eq!(io.to_string(), "launching the game: boom");
        assert_eq!(io.source().expect("should chain").to_string(), "boom");
    }

    #[test]
    fn each_token_differs_from_the_last() {
        let a = fresh_token();
        let b = fresh_token();
        assert_ne!(a, b);
    }
}
