//! The session: launching the game, and one request at a time over it.

use crate::session::config::BINARY;
use crate::wire::command::Command;
use crate::wire::greeting::parse_greeting;
use crate::wire::refusal::{WireFault, WireRefusal};
use crate::{Config, Error, Greeting, Reply, Response};
use serde_json::Value;
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::marker::PhantomData;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::process::{Child, Command as Proc, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sts2_core::{Answered, Construct, Decision, Play, Setup, Snapshot, Start};

/// A running game, and the process behind it.
///
/// The name is what it is *for*. [`Play`] and [`Construct`] are the whole of
/// what it offers a caller driving a run, and everything inherent below is
/// oracle-specific.
///
/// Dropping this closes the connection, which the game treats as a normal
/// shutdown, and then reaps the process. There is no `quit`: the protocol's is
/// what `Drop` sends.
pub struct Oracle<V: sts2_content_wire::WireContentVersion> {
    /// The build names no data here, so the seat is `Send` whatever it is.
    version: PhantomData<fn() -> V>,
    child: Child,
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    greeting: Greeting,
    command_timeout: Duration,
    /// The most recent reply, kept so the envelope stays reachable.
    last: Option<Response>,
}

impl<V: sts2_content_wire::WireContentVersion> Oracle<V> {
    /// Launches the game and waits for it to connect back.
    ///
    /// The game dials out. This binds an ephemeral port first and passes it to
    /// the game, so there is no port-allocation race and no connect-retry loop.
    pub fn launch(config: Config) -> Result<Self, Error> {
        Self::launch_as(config, "serve", &[])
    }

    /// Launches the game in a mode of the mod's, with settings of the mode's
    /// own, and waits for it to connect back.
    pub(crate) fn launch_as(
        config: Config,
        mode: &str,
        settings: &[(&str, String)],
    ) -> Result<Self, Error> {
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

        if let Some(home) = &config.data_home {
            super::config::seed_profile(home)
                .map_err(|e| Error::Io("copying the profile into the game's own home", e))?;
        }

        let mut proc = Proc::new(&binary);
        if let Some(home) = &config.data_home {
            proc.env("XDG_DATA_HOME", home);
        }
        proc.current_dir(&config.game_dir)
            .args(["--headless", "--force-steam=off"])
            .env("STS2_HEADLESS_MODE", mode)
            .envs(settings.iter().map(|(name, value)| (name, value)))
            .env("STS2_HEADLESS_PORT", port.to_string())
            .env("STS2_HEADLESS_TOKEN", &token)
            // Always. The digest-only mode saves bandwidth and log size, not
            // time -- H7 measures the two within 5% of each other, because a
            // decision costs frame-boundary latency rather than serialisation.
            // Paying ~200KB/s to make `state` unconditional is the better
            // trade, and keeps Observation and Pending off an Option.
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
                    last: None,
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

        greeting.built_from_our_source()?;

        // The startup deadline was for a game that had not booted yet; commands
        // get the shorter one.
        reader
            .get_ref()
            .set_read_timeout(Some(config.command_timeout))
            .map_err(|e| Error::Io("setting the command deadline", e))?;

        Ok((writer, reader, greeting))
    }

    /// What the game said about itself: which mod answered, which game build it
    /// is, and every command it accepts.
    pub fn greeting(&self) -> &Greeting {
        &self.greeting
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

    /// The most recent reply, as the protocol sent it.
    ///
    /// The envelope [`Play`] drops, none of which is a fact about the run.
    ///
    /// The digest is the reason this exists. The mod computes one over the
    /// state it built, and [`sts2_core::Observation::digest`] over what
    /// survived. Two runs the game calls different while this crate calls them
    /// the same is an observation missing a field.
    ///
    /// `None` before the first command.
    pub fn reported(&self) -> Option<&Response> {
        self.last.as_ref()
    }

    /// Puts one domain value on the wire in the protocol's own spelling.
    ///
    /// The conversion is the point of the boundary. Nothing in `sts2-core`
    /// knows how to serialize itself, so the mod's shape cannot leak out of
    /// this crate by accident.
    pub(crate) fn dispatch<'a>(
        &mut self,
        command: impl Into<Command<V>> + 'a,
    ) -> Result<Reply<V>, Error> {
        let wire: Command<V> = command.into();
        let line =
            serde_json::to_string(&wire).map_err(|e| Error::Encode(format!("{wire:?}"), e))?;

        parse_reply::<V>(&self.send_line(&line)?)
    }

    /// Sends a command this crate's types cannot vouch for.
    ///
    /// The typed API is exhaustive over what the tables know, which is why it
    /// cannot express a request the game refuses. Testing the refusal path needs
    /// to send one anyway.
    ///
    /// Nothing else reaches for this. A command the typed API cannot build and
    /// the game accepts is a gap in the content tables.
    pub fn send_raw(&mut self, command: &Value) -> Result<Reply<V>, Error> {
        let line =
            serde_json::to_string(command).map_err(|e| Error::Encode(format!("{command}"), e))?;

        parse_reply::<V>(&self.send_line(&line)?)
    }

    /// Sends one already-encoded line and returns the game's answer unparsed.
    ///
    /// For a relay, meaning something holding the game open on behalf of
    /// another process. A relay has no reason to decode a reply only to
    /// re-encode it. Re-encoding also needs a `Serialize` inverse for every
    /// response type, and `RewardKind` reads a flat wire form into a sum. That
    /// inverse is hand-written, and free to drift from the parser beside it.
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

    /// Keeps a reply's envelope, and turns it into the answer.
    pub(crate) fn receive(&mut self, reply: Reply<V>) -> Result<Answered<V>, Error> {
        super::receive(reply, &mut self.last)
    }

    /// How long a single command can take before [`Error::Timeout`].
    pub fn command_timeout(&self) -> Duration {
        self.command_timeout
    }

    /// The game process's id, for diagnostics and for checking it has gone.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl<V: sts2_content_wire::WireContentVersion> Play<V> for Oracle<V> {
    type Error = Error;

    /// Where the game is. This holder owns the process, so it has to be told.
    type Host = Config;

    /// Launches a game and starts a run on it.
    ///
    /// One run per process here: the child is this value's, and it dies with
    /// it. A holder that outlives a run is [`Session`](super::held::Session),
    /// where the daemon owns the game and says whether a run is already open.
    fn begin(config: Self::Host, start: &Start<V>) -> Result<Self, Error> {
        let mut oracle = Self::launch(config)?;
        let reply = oracle.dispatch(start)?;
        // The run's own first report is dropped. `observe` asks for it, and a
        // caller that wants it says so, the way it does after any other step.
        match super::receive(reply, &mut oracle.last)? {
            Ok(_) => Ok(oracle),
            Err(refusal) => Err(Error::NotStarted(refusal.to_string())),
        }
    }

    fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Error> {
        let reply = self.dispatch(decision)?;
        super::receive(reply, &mut self.last)
    }

    /// Straight down the socket rather than through `dispatch`: there
    /// is no [`Decision`] that spells this, which is the point of it being its
    /// own method.
    fn observe(&mut self) -> Result<Snapshot<V>, Error> {
        let reply = parse_reply::<V>(&self.send_line(r#"{"cmd":"observe"}"#)?)?;
        match super::receive(reply, &mut self.last)? {
            Ok(applied) => Ok(applied.snapshot()),
            Err(refusal) => Err(Error::NotObserved(refusal.to_string())),
        }
    }
}

impl<V: sts2_content_wire::WireContentVersion> Construct<V> for Oracle<V> {
    fn setup(&mut self, step: &Setup<V>) -> Result<Answered<V>, Error> {
        let reply = self.dispatch(step)?;
        super::receive(reply, &mut self.last)
    }
}

impl<V: sts2_content_wire::WireContentVersion> Drop for Oracle<V> {
    fn drop(&mut self) {
        // Closing the socket is the documented shutdown: the game sees EOF and
        // exits. Shutting down explicitly rather than relying on field drop
        // order, so the child is already on its way out before we wait for it.
        let _ = self.writer.shutdown(std::net::Shutdown::Both);
        reap(&mut self.child);
    }
}

impl<V: sts2_content_wire::WireContentVersion> fmt::Debug for Oracle<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Oracle")
            .field("pid", &self.child.id())
            .field("game", &self.greeting.game)
            .field("mod", &self.greeting.identity)
            .finish_non_exhaustive()
    }
}

/// Waits for the game to dial in, giving up if the deadline passes or the
/// process dies first.
///
/// Polling rather than a blocking accept. A game that crashes during startup
/// otherwise hangs the caller until the deadline for no reason.
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
        // A reset is the same, from a game that died with a command unread.
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => Err(Error::GameGone),
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
/// The discriminator is read before deserializing. Each kind of message is
/// routed by what it says it is, not by which struct accepts it.
///
/// Public because [`Oracle::send_line`] hands a relay the raw line, and whatever
/// reads it must decide response-or-refusal the same way.
///
/// A `fault` is neither half of the pair, and comes back as an [`Error`].
pub fn parse_reply<V: sts2_content_wire::WireContentVersion>(
    line: &str,
) -> Result<Reply<V>, Error> {
    let decode = |e| Error::Decode(line.to_string(), e);
    let value: Value = serde_json::from_str(line).map_err(decode)?;

    match value.get("type").and_then(Value::as_str) {
        Some("error") => {
            let wire: WireRefusal = serde_json::from_value(value).map_err(decode)?;
            Ok(Err(wire.into_domain::<V>()?))
        }
        Some("fault") => {
            let fault: WireFault = serde_json::from_value(value).map_err(decode)?;
            Err(Error::Faulted {
                cmd: fault.cmd,
                error: fault.error,
                message: fault.message,
            })
        }
        _ => Ok(Ok(serde_json::from_value(value).map_err(decode)?)),
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
/// Not a secret and not trying to be. It distinguishes our game from something
/// else that dialled the port, which is the whole job. Anything on this machine
/// can read the environment it is passed in.
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
    use crate::wire::greeting::MOD_IDENTITY;
    use sts2_content::v0_107_1::V0_107_1;

    type V = V0_107_1;

    /// A greeting from the mod this client was built against.
    ///
    /// Built rather than written out, because the identity is a hash of the
    /// mod's source and so is only known at build time. A literal here is a
    /// constant validating itself.
    fn hello() -> String {
        format!(
            r#"{{"type":"hello","mod":"{MOD_IDENTITY}","game":"v0.107.1",
            "commit":"abc1234","assemblyHash":1692500715,"contentHash":42,
            "commands":["start","quit"]}}"#
        )
    }

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
    /// The peer connects *before* the accept, sitting in the listener's
    /// backlog. That keeps this single-threaded, and keeps its stream alive for
    /// the whole handshake without a sleep to hold it open.
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
            Oracle::<V>::connect(&listener, &mut child, token, &quick(Duration::from_secs(5)))
                .map(|(_, _, hello)| hello);
        reap(&mut child);
        result
    }

    #[test]
    fn a_matching_token_and_greeting_get_through() {
        let greeting = against("tok", &["tok", &hello()]).expect("handshake should succeed");
        assert_eq!(greeting.identity.as_deref(), Some(MOD_IDENTITY));
        assert_eq!(greeting.game, "v0.107.1");
        assert_eq!(greeting.assembly_hash, 1_692_500_715);
        assert_eq!(greeting.commands, ["start", "quit"]);
    }

    #[test]
    fn a_stranger_on_our_port_is_refused() {
        let err = against("tok", &["not-our-token", &hello()]).expect_err("should refuse");
        assert!(matches!(err, Error::WrongPeer), "got {err:?}");
    }

    #[test]
    fn the_first_message_has_to_be_a_greeting() {
        let err = against("tok", &["tok", r#"{"type":"bye"}"#]).expect_err("should refuse");
        assert!(matches!(err, Error::NoGreeting(_)), "got {err:?}");
    }

    /// The check this whole mechanism exists for. A mod that was edited and
    /// never redeployed is caught at the handshake, rather than as a decode
    /// failure deep in a run.
    #[test]
    fn a_mod_we_were_not_built_against_is_refused() {
        let stale = r#"{"type":"hello","mod":"0000000000000000","game":"v0.107.1",
            "commit":"c","assemblyHash":1,"contentHash":1,"commands":[]}"#;
        let err = against("tok", &["tok", stale]).expect_err("should refuse");
        let Error::WrongMod { expected, found } = err else {
            panic!("expected a mod mismatch, got {err:?}");
        };
        assert_eq!(expected, MOD_IDENTITY);
        assert_eq!(found, "0000000000000000");
    }

    /// A mod from before the greeting carried an identity cannot be ours. It
    /// says so, rather than failing as a greeting missing a field.
    #[test]
    fn a_mod_that_does_not_identify_itself_is_refused_by_name() {
        let old = r#"{"type":"hello","protocol":1,"game":"v0.107.1","commit":"c",
            "assemblyHash":1,"contentHash":1,"commands":[]}"#;
        let err = against("tok", &["tok", old]).expect_err("should refuse");
        assert!(
            matches!(&err, Error::WrongMod { found, .. } if found == "a mod too old to say"),
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
        let err =
            Oracle::<V>::connect(&listener, &mut child, "tok", &quick(Duration::from_secs(5)))
                .map(|_| ())
                .expect_err("should fail");
        reap(&mut child);
        assert!(matches!(err, Error::GameGone), "got {err:?}");
    }

    /// A game that dies during startup says so immediately, rather than making
    /// the caller wait out the whole deadline.
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
        let err = Oracle::<V>::launch(config)
            .map(|_| ())
            .expect_err("should fail");
        assert!(matches!(err, Error::GameNotFound(_)), "got {err:?}");
    }

    /// These strings are what a caller sees when something goes wrong.
    /// `thiserror` generates them from attributes rather than a match, so it is
    /// worth pinning that they still read as sentences and still chain.
    #[test]
    fn errors_say_something_useful() {
        use std::error::Error as _;

        let missing = Error::GameNotFound("/nope/SlayTheSpire2".into());
        assert_eq!(missing.to_string(), "no game binary at /nope/SlayTheSpire2");
        assert!(missing.source().is_none());

        let mismatch = Error::WrongMod {
            expected: "aaaa".into(),
            found: "bbbb".into(),
        };
        assert_eq!(
            mismatch.to_string(),
            "the game is running mod bbbb, this client was built against aaaa; redeploy the mod"
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
