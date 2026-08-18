//! The session: launching the game, and one request at a time over it.

use crate::session::config::BINARY;
use crate::wire::greeting::{PROTOCOL_VERSION, parse_greeting};
use crate::{Command, Config, Error, Greeting, Reply};
use serde_json::Value;
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::marker::PhantomData;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::process::{Child, Command as Proc, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sts2_content::ContentVersion;

/// A running game, and the process behind it.
///
/// Dropping this closes the connection, which the game treats as a normal
/// shutdown, and then reaps the process — so a client that dies does not leave a
/// game behind.
pub struct Game<V: ContentVersion> {
    version: PhantomData<V>,
    child: Child,
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    greeting: Greeting,
    command_timeout: Duration,
}

impl<V: ContentVersion> Game<V> {
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
            Ok((writer, reader, greeting)) => match greeting.describes(&V::VERSION) {
                Ok(()) => Ok(Self {
                    version: PhantomData,
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
            },
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
    pub fn send(&mut self, command: &Command<V>) -> Result<Reply, Error> {
        let line =
            serde_json::to_string(command).map_err(|e| Error::Encode(format!("{command:?}"), e))?;

        parse_reply(&self.send_line(&line)?)
    }

    /// Sends a command this crate's types cannot vouch for.
    ///
    /// The typed API is exhaustive over what the tables know, which is exactly
    /// why it cannot express a request the game should refuse — an encounter
    /// that does not exist, a `RoomType` codegen deliberately drops. Testing the
    /// refusal path needs to send those, and that is a different operation from
    /// driving the game, so it says so here rather than in the type of every
    /// field.
    ///
    /// Nothing else should reach for this. A command that the typed API cannot
    /// build and the game *should* accept is a gap in the content tables, and
    /// the fix is there.
    pub fn send_raw(&mut self, command: &Value) -> Result<Reply, Error> {
        let line =
            serde_json::to_string(command).map_err(|e| Error::Encode(format!("{command}"), e))?;

        parse_reply(&self.send_line(&line)?)
    }

    /// Sends one already-encoded line and returns the game's answer unparsed.
    ///
    /// For a relay — something holding the game open on behalf of another
    /// process — which has no reason to decode a reply only to re-encode it.
    /// Re-encoding would also need a `Serialize` inverse for every response
    /// type, and `RewardKind` reads a flat wire form into a sum, so that inverse
    /// would be hand-written and free to drift from the parser beside it.
    pub fn send_line(&mut self, line: &str) -> Result<String, Error> {
        self.write_line(line)?;
        read_line(&mut self.reader)
    }

    fn write_line(&mut self, line: &str) -> Result<(), Error> {
        self.writer
            .write_all(line.as_bytes())
            .and_then(|()| self.writer.write_all(b"\n"))
            .and_then(|()| self.writer.flush())
            .map_err(|e| Error::Io("sending a command", e))
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

impl<V: ContentVersion> Drop for Game<V> {
    fn drop(&mut self) {
        // Closing the socket is the documented shutdown: the game sees EOF and
        // exits. Shutting down explicitly rather than relying on field drop
        // order, so the child is already on its way out before we wait for it.
        let _ = self.writer.shutdown(std::net::Shutdown::Both);
        reap(&mut self.child);
    }
}

impl<V: ContentVersion> fmt::Debug for Game<V> {
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
/// Reads one reply line the way [`Game::send`] does.
///
/// Public because [`Game::send_line`] hands a relay the raw line, and whatever
/// finally reads it must decide response-or-refusal the same way. `Reply` is a
/// `Result`, so deriving that decision from serde alone would look for an
/// externally tagged `{"Ok": …}` and be wrong about every message.
pub fn parse_reply(line: &str) -> Result<Reply, Error> {
    let decode = |e| Error::Decode(line.to_string(), e);
    let value: Value = serde_json::from_str(line).map_err(decode)?;

    if value.get("type").and_then(Value::as_str) == Some("error") {
        return Ok(Err(serde_json::from_value(value).map_err(decode)?));
    }
    Ok(Ok(serde_json::from_value(value).map_err(decode)?))
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

#[cfg(test)]
mod handshake {
    use super::*;
    use sts2_content::v0_107_1::V0_107_1;

    type V = V0_107_1;

    const HELLO: &str = r#"{"type":"hello","protocol":1,"game":"v0.107.1",
        "commit":"abc1234","assemblyHash":1692500715,"contentHash":42,
            "commands":["start","quit"]}"#;

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
        let result =
            Game::<V>::connect(&listener, &mut child, token, &quick(Duration::from_secs(5)))
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
        let future = r#"{"type":"hello","protocol":2,"game":"v9","commit":"c","assemblyHash":1,
            "contentHash":1,"commands":[]}"#;
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
        let err = Game::<V>::connect(&listener, &mut child, "tok", &quick(Duration::from_secs(5)))
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
        let err = Game::<V>::launch(config)
            .map(|_| ())
            .expect_err("should fail");
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
