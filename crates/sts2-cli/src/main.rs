//! `sts2`, one game held open, one command per invocation.
//! The game boots in seconds and holds every bit of run state, so a command cannot be a whole process. A daemon owns it and each invocation is a short conversation with that daemon. This means a script, or an agent, can send a command, read the answer, and decide the next one. A REPL cannot offer that: its caller has to commit to every input before seeing any output.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::time::Duration;
use sts2_cli::Readable;
use sts2_cli::cli::{Play, choose};
use sts2_cli::session::{Answer, DEFAULT_IDLE_TIMEOUT, Request, client, daemon, socket_path};
use sts2_content::v0_107_1::V0_107_1;
use sts2_headless::{Command, Config, Response};

/// The build this drives.
///
/// Named once. Every id follows from it through `ContentVersion`, so pointing the tool at another build is a one-line change.
type V = V0_107_1;

#[derive(Debug, Parser)]
#[command(name = "sts2", about = "Drive a headless Slay the Spire 2.", version)]
struct Cli {
    #[command(subcommand)]
    command: Top,
}

#[derive(Debug, Subcommand)]
enum Top {
    #[command(flatten)]
    Play(Play),

    /// Runs the daemon that holds the game.
    ///
    /// Usually not typed: `sts2 start` spawns one in the background. Run it in the foreground to watch what the game does.
    Serve {
        /// Detach and return, leaving the daemon running.
        #[arg(long)]
        detach: bool,
        /// Seconds of quiet before the daemon gives up and exits.
        #[arg(long, default_value_t = DEFAULT_IDLE_TIMEOUT.as_secs())]
        idle_timeout: u64,
    },

    /// Takes a numbered option, whichever way the game is asking for it.
    ///
    /// Room options, card selections and reward screens are three protocol mechanisms for one idea. Which applies is the game's state rather than the user's intent. This looks, then does the right one.
    ///
    /// Scripts must prefer explicit commands. `answer cards 0` fails loudly when the game asks something else, while this command acts on whatever is pending.
    Choose {
        /// A plain index, or `N.M` for a card inside a card reward.
        #[arg(value_delimiter = ' ', num_args = 1..)]
        indices: Vec<sts2_cli::cli::Taking>,
    },

    /// Reports whether a game is up, and how close it is to timing out.
    Status,

    /// Shuts the game and the daemon down.
    Stop,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Top::Serve {
            detach,
            idle_timeout,
        } => {
            let idle = Duration::from_secs(idle_timeout);
            if detach {
                daemon::spawn_detached(idle)?;
                println!("serving at {}", socket_path().display());
                return Ok(());
            }
            daemon::serve::<V>(game_config()?, idle)
        }

        Top::Choose { indices } => {
            // One extra round trip against a daemon that is already up, to read
            // what the game is waiting for. `observe` does not advance
            // anything, which is what makes looking first safe.
            let seen = send(&Command::<V>::Observe)?;
            let awaiting = choose::awaiting(&seen)?;
            let command = choose::command::<V>(&awaiting, &indices)?;

            eprintln!("({})", choose::describe(&awaiting));
            report(client::ask(&Request::Game {
                command: serde_json::to_value(&command)?,
            })?)
        }

        Top::Status => match client::ask(&Request::Status)? {
            Answer::Status {
                pid,
                game,
                idle_seconds,
                idle_timeout_seconds,
            } => {
                println!(
                    "running  pid {pid}\ngame     {game}\nidle     {idle_seconds}s of {idle_timeout_seconds}s"
                );
                Ok(())
            }
            other => bail!("unexpected answer to status: {other:?}"),
        },

        Top::Stop => {
            if !client::running() {
                println!("nothing running");
                return Ok(());
            }
            client::ask(&Request::Stop)?;
            println!("stopped");
            Ok(())
        }

        Top::Play(play) => {
            let command = play.command::<V>()?;
            let request = Request::Game {
                command: serde_json::to_value(&command)?,
            };

            // Only `start` can bring a game up. Any other command against no
            // game is a mistake worth naming, not a twenty-second pause
            // followed by a refusal.
            let answer = match play {
                Play::Start(_) => client::ask_or_start(&request, DEFAULT_IDLE_TIMEOUT)?,
                _ => client::ask(&request)?,
            };

            report(answer)
        }
    }
}

/// Sends one command and returns what the game said, refusals as errors.
fn send(command: &Command<V>) -> Result<Response> {
    let answer = client::ask(&Request::Game {
        command: serde_json::to_value(command)?,
    })?;

    match answer {
        Answer::Reply { line } => match sts2_headless::parse_reply(&line)? {
            Ok(response) => Ok(response),
            Err(refusal) => bail!("{refusal}"),
        },
        Answer::Failed { message } => bail!("{message}"),
        other => bail!("unexpected answer: {other:?}"),
    }
}

fn report(answer: Answer) -> Result<()> {
    match answer {
        Answer::Reply { line } => {
            let reply = sts2_headless::parse_reply(&line).context("reading the game's reply")?;
            match reply {
                Ok(response) => {
                    println!("{}", Readable(&response));
                    Ok(())
                }
                // A refusal is the game answering, not this failing, but it is
                // the wrong exit status for a script, so it is an error here.
                Err(refusal) => bail!("{refusal}"),
            }
        }
        Answer::Failed { message } => bail!("{message}"),
        other => bail!("unexpected answer: {other:?}"),
    }
}

fn game_config() -> Result<Config> {
    let dir =
        sts2_headless::game_dir_from_env().context("set STS2_GAME_DIR to the game's directory")?;

    Ok(Config::new(dir))
}
