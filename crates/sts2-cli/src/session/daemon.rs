//! The process that holds the game open.
//!
//! One game, one socket, one client at a time. Requests are served in the order
//! they arrive because the game is a single state machine — two commands in
//! flight would be two people playing one run.

use super::{Answer, Request, socket_path};
use anyhow::{Context, Result, bail};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::{Duration, Instant};
use sts2_content::ContentVersion;
use sts2_headless::{Config, Game};

/// Runs until told to stop, or until nobody has asked for anything in a while.
///
/// The timeout is not a nicety. A client is one short-lived process per command,
/// so nothing observes the daemon's health and nothing tidies up after a client
/// that crashed — without it, a forgotten game sits holding memory until the
/// machine reboots.
pub fn serve<V: ContentVersion>(config: Config, idle_timeout: Duration) -> Result<()> {
    let path = socket_path();

    // A socket left by a daemon that died is not a daemon. Removing it is safe
    // because binding is what proves the previous one is gone: if one is still
    // listening, the connect in `client::probe` would have succeeded and we
    // would not be here.
    let _ = std::fs::remove_file(&path);
    let listener =
        UnixListener::bind(&path).with_context(|| format!("binding {}", path.display()))?;
    listener
        .set_nonblocking(true)
        .context("the listener has to be pollable so the timeout can fire")?;

    let mut game: Option<Game<V>> = None;
    let mut last = Instant::now();

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                last = Instant::now();
                if !serve_one::<V>(stream, &config, &mut game, idle_timeout, last)? {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if last.elapsed() >= idle_timeout {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e).context("accepting a client"),
        }
    }

    let _ = std::fs::remove_file(&path);
    Ok(())
}

/// Handles one connection. Returns `false` when the daemon should stop.
fn serve_one<V: ContentVersion>(
    stream: UnixStream,
    config: &Config,
    game: &mut Option<Game<V>>,
    idle_timeout: Duration,
    last: Instant,
) -> Result<bool> {
    stream.set_nonblocking(false)?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(true);
    }

    let request: Request = serde_json::from_str(line.trim())
        .with_context(|| format!("reading a request: {}", line.trim()))?;

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
            },
            true,
        ),
        Request::Game { command } => (relay::<V>(config, game, command), true),
    };

    writeln!(writer, "{}", serde_json::to_string(&answer)?)?;
    writer.flush()?;

    Ok(keep_going)
}

/// Sends one command to the game, launching it on the first one.
///
/// Launching lazily means `sts2 start` is the only command that pays the boot
/// cost, and a daemon spawned in the background is useful the moment it binds
/// rather than twenty seconds later.
fn relay<V: ContentVersion>(
    config: &Config,
    game: &mut Option<Game<V>>,
    command: serde_json::Value,
) -> Answer {
    if game.is_none() {
        match Game::<V>::launch(config.clone()) {
            Ok(fresh) => *game = Some(fresh),
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
        Ok(line) => Answer::Reply { line },
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

/// Re-runs this binary as a detached daemon.
pub fn spawn_detached(idle_timeout: Duration) -> Result<()> {
    let exe = std::env::current_exe().context("finding this binary")?;
    let child = std::process::Command::new(exe)
        .arg("serve")
        .arg("--idle-timeout")
        .arg(idle_timeout.as_secs().to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning the daemon")?;

    // Waiting for the socket rather than for the process: a daemon that has not
    // bound yet is one a client would fail to reach.
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if UnixStream::connect(socket_path()).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    bail!(
        "the daemon (pid {}) did not bind a socket within ten seconds",
        child.id()
    )
}
