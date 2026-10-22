//! `sts2`: one game held open, one command per invocation.
//!
//! A command is a whole process, so a script or an agent can send one, read the
//! answer, and decide the next. A REPL does not offer that. Its caller has to
//! commit to every input before seeing any output.
//!
//! # What this front end knows about the game
//!
//! That it implements [`sts2_core::Construct`]. Nothing else.
//!
//! It does not know that the oracle boots in seconds, that something holds it
//! between two invocations, or that a unix socket is involved. Those are facts
//! about *that* implementation, and [`sts2_cli::oracle`] keeps them behind
//! [`Held`].
//!
//! # What is left in the binary
//!
//! The command line itself: what clap accepts, which group each subcommand
//! belongs to, and how an answer is printed. [`sts2_cli::control`] performs the
//! commands, against anything that plays the game.

use clap::{Parser, Subcommand};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;
use sts2_cli::cli::{Play, StartArgs};
use sts2_cli::control::{self, Outcome};
use sts2_cli::oracle::{self, Held};
use sts2_cli::tui;
use sts2_core::Play as _;
use sts2_headless::Session;
use sts2_headless::daemon::DEFAULT_IDLE_TIMEOUT;

/// The implementation this drives, named once.
///
/// Everything below reaches it through [`sts2_core::Construct`], so the day
/// there is a simulator this line is what changes. `Session` is the oracle held
/// open by a daemon. `sts2_headless::Oracle` is one this process owns and kills
/// on the way out.
type Game<V> = Session<V>;

/// What holds the game, named once beside it.
///
/// The oracle's daemon and socket live behind this. A simulator needs no such
/// line, because there is nothing to hold.
type Host<V> = Held<V>;

#[derive(Debug, Parser)]
#[command(name = "sts2", about = "Drive a headless Slay the Spire 2.", version)]
struct Cli {
    /// Also print the RNG stream counters.
    ///
    /// Off by default, because a player cannot use them. On for looking at a
    /// divergence, where a counter that moved when nothing moved it is the
    /// evidence.
    #[arg(long, global = true)]
    rng: bool,

    /// Which pinned build to drive, as its lock under `mod/games/` names it.
    ///
    /// Defaults to `STS2_GAME_VERSION`, which is what `.envrc` exports and what
    /// picks the game directory. One daemon per build, so two can be held open
    /// at once and a command cannot reach the wrong one.
    #[arg(long, global = true, value_name = "VERSION")]
    game_version: Option<String>,

    /// Nothing at all opens the game, held on the screen and played with the
    /// keyboard. Subcommands form the non-interactive surface a script or agent drives:
    /// one move per process, so the caller can read each answer before
    /// deciding the next.
    #[command(subcommand)]
    command: Option<Top>,
}

#[derive(Debug, Subcommand)]
enum Top {
    /// Starts, resumes, records, or stops a run.
    Run {
        #[command(subcommand)]
        command: RunCommand,
    },
    /// Inspects the current run without changing it.
    Show {
        #[command(subcommand)]
        command: ShowCommand,
    },
    /// Takes an action in the current run.
    Act {
        #[command(subcommand)]
        command: ActCommand,
    },
    /// Constructs a scenario outside ordinary play.
    Setup {
        #[command(subcommand)]
        command: SetupCommand,
    },
    /// Manages the process that holds the game open.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
}

#[derive(Debug, Subcommand)]
enum RunCommand {
    /// Begins a run, launching the game if it is not already up.
    Start(sts2_cli::cli::StartArgs),
    /// Starts or stops writing the run down, mid-session.
    Record {
        /// Where to write. Omit to stop recording.
        file: Option<PathBuf>,
    },
    /// Picks a recorded run back up where it left off.
    Replay {
        file: PathBuf,
        /// Stop after this many steps, to land mid-run.
        #[arg(long, value_name = "N")]
        steps: Option<usize>,
        /// Write the run from here on to this file.
        #[arg(long, value_name = "FILE")]
        record: Option<PathBuf>,
    },
    /// Reports whether a game is up, and how close it is to timing out.
    Status,
    /// Shuts the game and the daemon down.
    Stop,
}

#[derive(Debug, Subcommand)]
enum ShowCommand {
    /// Prints the current state without changing it.
    State,
    /// Draws the act: every room, and which lead to which.
    Map,
}

#[derive(Debug, Subcommand)]
enum ActCommand {
    /// Takes a numbered option, whichever way the game is asking for it.
    Choose {
        /// A plain index, or N.M for a card inside a card reward.
        #[arg(value_delimiter = ' ', num_args = 1..)]
        indices: Vec<sts2_cli::cli::Taking>,
    },
    /// Moves to a map point.
    Travel { col: u8, row: u8 },
    /// Enters a room by type, such as Shop.
    EnterRoom { room: String },
    /// Plays a card from hand.
    PlayCard {
        index: u8,
        #[arg(long)]
        target: Option<u32>,
    },
    /// Ends the player turn.
    EndTurn,
    /// Drinks a potion from the belt.
    UsePotion {
        index: u8,
        #[arg(long)]
        target: Option<u32>,
    },
    /// Leaves a finished act for the next one.
    Proceed,
    /// Takes an offered option by index.
    ChooseOption { index: u8 },
    /// Buys the shop item at an index.
    Buy { index: u8 },
    /// Answers the pending question.
    #[command(subcommand)]
    Answer(sts2_cli::cli::Answering),
}

#[derive(Debug, Subcommand)]
enum SetupCommand {
    /// Jumps straight into a named encounter, off the map.
    Fight { encounter: String },
    /// Jumps straight to a named event, off the map.
    Event { event: String },
    /// Puts a card, relic or potion into the run.
    Grant {
        #[arg(
            long,
            conflicts_with_all = ["relic", "potion"],
            required_unless_present_any = ["relic", "potion"],
        )]
        card: Option<String>,
        #[arg(long, conflicts_with = "potion")]
        relic: Option<String>,
        #[arg(long)]
        potion: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum DaemonCommand {
    /// Runs the daemon that holds the game.
    Serve {
        /// Detach and return, leaving the daemon running.
        #[arg(long)]
        detach: bool,
        /// Seconds of quiet before the daemon gives up and exits.
        #[arg(long, default_value_t = DEFAULT_IDLE_TIMEOUT.as_secs())]
        idle_timeout: u64,
    },
}

/// Why an invocation did not do what it asked.
#[derive(Debug, thiserror::Error)]
enum Error<V: sts2_content_wire::WireContentVersion> {
    #[error(transparent)]
    Cli(#[from] sts2_cli::cli::Error),
    #[error(transparent)]
    Control(#[from] control::Error<V>),
    #[error(transparent)]
    Oracle(#[from] oracle::Error),
    #[error(transparent)]
    Tui(#[from] tui::Error),
    #[error("starting the run: {0}")]
    Start(sts2_headless::Error),
    #[error("--steps 0 stops before the first step, which replays nothing")]
    NoSteps,
}

type Result<T, V> = std::result::Result<T, Error<V>>;

/// One invocation, grouped by what it addresses.
///
/// Starting creates a run. A [`control::Command`] addresses one that exists.
/// A [`Machinery`] command addresses what holds the oracle between invocations.
#[derive(Debug)]
enum Command {
    /// Begins a run, which creates the thing that play commands use.
    Start(StartArgs),
    Run(control::Command),
    Held(Machinery),
}

/// A command about what holds the game, rather than about the run.
#[derive(Debug)]
enum Machinery {
    Serve {
        detach: bool,
        idle_timeout: u64,
    },
    Record {
        file: Option<PathBuf>,
    },
    Replay {
        file: PathBuf,
        steps: Option<usize>,
        record: Option<PathBuf>,
    },
    Status,
    Stop,
}

/// Puts a subcommand into the half that answers it.
fn grouped(top: Top) -> Command {
    let run = |play| Command::Run(control::Command::Step(play));

    match top {
        Top::Run { command } => match command {
            RunCommand::Start(args) => Command::Start(args),
            RunCommand::Record { file } => Command::Held(Machinery::Record { file }),
            RunCommand::Replay {
                file,
                steps,
                record,
            } => Command::Held(Machinery::Replay {
                file,
                steps,
                record,
            }),
            RunCommand::Status => Command::Held(Machinery::Status),
            RunCommand::Stop => Command::Held(Machinery::Stop),
        },
        Top::Show { command } => match command {
            ShowCommand::State => Command::Run(control::Command::Observe),
            ShowCommand::Map => Command::Run(control::Command::Map),
        },
        Top::Act { command } => match command {
            ActCommand::Choose { indices } => Command::Run(control::Command::Choose(indices)),
            ActCommand::Travel { col, row } => run(Play::Travel { col, row }),
            ActCommand::EnterRoom { room } => run(Play::EnterRoom { room }),
            ActCommand::PlayCard { index, target } => run(Play::PlayCard { index, target }),
            ActCommand::EndTurn => run(Play::EndTurn),
            ActCommand::UsePotion { index, target } => run(Play::UsePotion { index, target }),
            ActCommand::Proceed => run(Play::Proceed),
            ActCommand::ChooseOption { index } => run(Play::ChooseOption { index }),
            ActCommand::Buy { index } => run(Play::Buy { index }),
            ActCommand::Answer(answer) => run(Play::Answer(answer)),
        },
        Top::Setup { command } => run(match command {
            SetupCommand::Fight { encounter } => Play::Fight { encounter },
            SetupCommand::Event { event } => Play::Event { event },
            SetupCommand::Grant {
                card,
                relic,
                potion,
            } => Play::Grant {
                card,
                relic,
                potion,
            },
        }),
        Top::Daemon { command } => match command {
            DaemonCommand::Serve {
                detach,
                idle_timeout,
            } => Command::Held(Machinery::Serve {
                detach,
                idle_timeout,
            }),
        },
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let version = cli
        .game_version
        .clone()
        .or_else(|| std::env::var("STS2_GAME_VERSION").ok())
        .unwrap_or_else(|| "v0.107.1".to_string());

    // One arm per build `sts2-content` compiles. Everything below is generic
    // over it, so this is the only place that names one.
    match version.as_str() {
        "v0.107.1" => exit(drive::<sts2_content::v0_107_1::V0_107_1>(cli)),
        "v0.111.0" => exit(drive::<sts2_content::v0_111_0::V0_111_0>(cli)),
        other => {
            eprintln!("no tables for game version {other:?}; try v0.107.1 or v0.111.0");
            ExitCode::FAILURE
        }
    }
}

/// The exit status for how a command went, with the error on stderr.
fn exit(done: std::result::Result<(), impl Display>) -> ExitCode {
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn drive<V: sts2_content_wire::WireContentVersion>(cli: Cli) -> Result<(), V> {
    let show = Show { rng: cli.rng };
    let held: Host<V> = Held::new(serving);

    let Some(top) = cli.command else {
        return play(&held);
    };

    match grouped(top) {
        Command::Held(Machinery::Serve {
            detach,
            idle_timeout,
        }) => Ok(held.serve(detach, Duration::from_secs(idle_timeout))?),

        Command::Held(Machinery::Record { file }) => said(held.record(file)?),
        Command::Held(Machinery::Status) => said(held.status()?),
        Command::Held(Machinery::Stop) => said(held.stop()?),

        Command::Held(Machinery::Replay {
            file,
            steps,
            record,
        }) => replay(&held, &file, steps, record, show),

        Command::Start(args) => start(&held, &args, show),

        Command::Run(command) => {
            let mut game = held.attached();
            let outcome = control::perform(&mut game, &command)?;
            report(&game, outcome, show)
        }
    }
}

/// Starts a run and prints the state it opened on.
fn start<V: sts2_content_wire::WireContentVersion>(
    held: &Host<V>,
    args: &StartArgs,
    show: Show,
) -> Result<(), V> {
    // Read before anything is brought up, so a mistyped seed or character
    // leaves behind neither a daemon nor an empty recording.
    let opening = args.start::<V>()?;

    held.wake(DEFAULT_IDLE_TIMEOUT)?;

    // Before the run starts, so the daemon's recording opens with it. A
    // recording that does not begin at a `start` is not a run.
    if let Some(path) = args.recording()? {
        oracle::begin_recording(&held.attached(), path, false)?;
    }

    // Starting is not a step, so it does not go through `perform`. It is what
    // makes the thing `perform` sends to, and the daemon refuses a second one
    // while it is holding a run.
    let mut game = Game::<V>::begin((), &opening).map_err(Error::Start)?;
    let outcome = Outcome::Seen {
        snapshot: Box::new(control::look(&mut game)?),
        decided: None,
    };
    report(&game, outcome, show)
}

/// Opens the game on the screen.
///
/// The game is not started for this. A run is begun with `sts2 run start`,
/// which is one line and says what it does. Bringing the game up on a bare
/// `sts2` means twenty seconds of nothing, followed by a screen with no run in
/// it. It also gives the player no way to say which seed they wanted.
fn play<V: sts2_content_wire::WireContentVersion>(held: &Host<V>) -> Result<(), V> {
    // The daemon comes up. The *game* does not, so a bare `sts2` opens on the
    // setup screen straight away and the game boots while the player is picking
    // a character.
    let fresh = held.wake(DEFAULT_IDLE_TIMEOUT)?;

    let game = held.attached();
    let options = tui::Options {
        recording: game.status().ok().and_then(|status| status.recording),
    };
    // What run to draw, and what to do when one ends, is this front end's
    // business rather than the screen's. The screen draws a run and says how it
    // ended.
    //
    // A daemon that was already up is holding a run, so that one is attached
    // to. A fresh one is holding nothing, and the setup screen is what asks
    // for the first.
    let mut attached = (!fresh).then_some(game);
    ratatui::run(|terminal| {
        let mut screen = tui::setup::Screen::<V>::default();
        loop {
            let game = match attached.take() {
                Some(game) => game,
                None => match screen.collect(terminal)? {
                    Some(start) => Game::<V>::begin((), &start).map_err(Error::Start)?,
                    None => return Ok(()),
                },
            };

            // A run that goes away leaves the setup screen, so the daemon
            // idling out mid-session is a new run rather than a dead window.
            if tui::play(terminal, game, options.clone())? == tui::Ended::Quit {
                return Ok(());
            }
        }
    })
}

/// What the caller asked to be shown beyond the game itself.
///
/// One flag today. It is a struct rather than a bare `bool` because the call
/// sites read `report(answer, show)` either way. The next one of these is then
/// not a second positional boolean.
#[derive(Debug, Clone, Copy)]
struct Show {
    rng: bool,
}

/// Prints a whole answer that is words rather than a state.
fn said<V: sts2_content_wire::WireContentVersion>(words: String) -> Result<(), V> {
    println!("{words}");
    Ok(())
}

/// Prints what one command produced, and anything that went wrong beside it.
///
/// The warning is the session failing rather than the game. A recording that
/// stopped means the move happened and the file did not. Saying nothing about
/// that is how a run gets played out under the belief it was being kept.
fn report<V: sts2_content_wire::WireContentVersion>(
    game: &Game<V>,
    outcome: Outcome<V>,
    show: Show,
) -> Result<(), V> {
    if let Some(warning) = game.warned() {
        eprintln!("(warning: {warning})");
    }
    match outcome {
        Outcome::Seen { snapshot, .. } => {
            println!("{}", oracle::readable(game, &snapshot, show.rng));
        }
        Outcome::Said(words) => print!("{words}"),
    }
    Ok(())
}

/// The command that re-enters this binary as the daemon.
///
/// Built here because *which binary serves* is this front end's fact, not
/// `sts2-headless`'s. The crate owns the socket and the loop, and this owns the
/// knowledge of the subcommand to come back through.
///
/// The subcommand is spelled the way [`Top`] groups it. It is a command line
/// like any other, and a test below parses it rather than trusting it.
fn serving(
    idle: Duration,
    version: &str,
) -> std::result::Result<std::process::Command, oracle::Error> {
    let exe = std::env::current_exe().map_err(oracle::Error::Exe)?;
    let mut command = std::process::Command::new(exe);
    command
        .args(["daemon", "serve"])
        .arg("--idle-timeout")
        .arg(idle.as_secs().to_string())
        // The daemon serves one build, and it has to be the one asked for. Not
        // inherited from the environment: the client may have been pointed at
        // another build with `--game-version`, and the daemon it starts must
        // agree with the client that started it.
        .arg("--game-version")
        .arg(version);
    Ok(command)
}

/// Whether two paths name the same existing file.
///
/// The file being replayed always exists, having just been read. A `--record`
/// target that does not exist is therefore not it. So a failure to resolve is a
/// `false` rather than a case to handle.
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Sends a recorded run's steps into a fresh game, and prints where it ended.
fn replay<V: sts2_content_wire::WireContentVersion>(
    held: &Host<V>,
    file: &Path,
    stop_after: Option<usize>,
    record: Option<PathBuf>,
    show: Show,
) -> Result<(), V> {
    let recording = oracle::recorded::<V>(file)?;
    let steps = match stop_after {
        Some(n) if n < recording.len() => &recording.steps()[..n],
        _ => recording.steps(),
    };

    if steps.is_empty() {
        return Err(Error::NoSteps);
    }

    let mut game = held.empty()?;

    // Carrying the replayed file on means appending to it: the steps about to
    // be sent are already there, so it starts *after* the replay. Anything else
    // is a fresh file that has to hold the whole run, so it starts before and
    // catches the steps as they go by.
    //
    // Appending is not only tidier. A replay that fails partway would otherwise
    // truncate the very run somebody opened it to look at.
    let carry_on = record
        .as_ref()
        .is_some_and(|path| same_file(file, path) && steps.len() == recording.len());

    if let Some(path) = &record
        && !carry_on
    {
        oracle::begin_recording(&game, path.clone(), false)?;
    }

    let last = control::replay(&mut game, steps)?;

    // The source file is resumed *after*, because it already holds every step
    // just sent.
    //
    // `--steps` is a branch: writing into the source from there would drop the
    // steps that were not replayed. So a branch records only where it is told
    // to, and says it is not recording otherwise. Silently stopping is how a
    // session gets played out under the belief it was being kept.
    if carry_on {
        oracle::begin_recording(&game, file.to_path_buf(), true)?;
    } else if record.is_none() {
        eprintln!("(not recording — pass --record FILE to play on into one)");
    }

    match last {
        Some((decided, snapshot)) => report(
            &game,
            Outcome::Seen {
                snapshot: Box::new(snapshot),
                decided: Some(decided),
            },
            show,
        ),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, Machinery, Top, grouped, serving};
    use clap::Parser;
    use std::ffi::OsStr;
    use sts2_headless::daemon::DEFAULT_IDLE_TIMEOUT;

    /// The daemon re-enters this binary, so what it is spawned with has to be a
    /// command line this binary accepts.
    ///
    /// Nothing else checks the loop. A subcommand that moves under a group
    /// leaves the spawn parsing nothing. The socket stays unbound, and every
    /// `run start` reports a daemon that did not come up.
    #[test]
    fn the_daemon_is_spawned_with_a_command_line_this_binary_accepts() {
        let spawned = serving(DEFAULT_IDLE_TIMEOUT, "v0.107.1").unwrap();
        let argv = std::iter::once(OsStr::new("sts2")).chain(spawned.get_args());

        let parsed = Cli::try_parse_from(argv).expect("a command line sts2 accepts");

        let Some(Top::Daemon { .. }) = &parsed.command else {
            panic!("the daemon is spawned into something other than `daemon`");
        };
        assert!(matches!(
            grouped(parsed.command.unwrap()),
            Command::Held(Machinery::Serve { detach: false, .. })
        ));
    }

    /// The daemon serves the build the client asked for.
    ///
    /// It cannot be inherited from the environment. A client run with
    /// `--game-version` then starts a daemon holding a different build. The
    /// socket is one for each build, so the two never meet: every command
    /// starts a second daemon and reports no game.
    #[test]
    fn the_build_survives_the_spawn() {
        let spawned = serving(DEFAULT_IDLE_TIMEOUT, "v0.111.0").unwrap();
        let argv = std::iter::once(OsStr::new("sts2")).chain(spawned.get_args());

        let parsed = Cli::try_parse_from(argv).expect("a command line sts2 accepts");

        assert_eq!(parsed.game_version.as_deref(), Some("v0.111.0"));
    }

    /// Start and observe stay out of the enum whose variants become steps.
    #[test]
    fn starting_and_observing_are_commands_of_their_own() {
        let start = Cli::try_parse_from(["sts2", "run", "start", "SP1RE1", "IRONCLAD"]).unwrap();
        assert!(matches!(grouped(start.command.unwrap()), Command::Start(_)));

        let observe = Cli::try_parse_from(["sts2", "show", "state"]).unwrap();
        assert!(matches!(
            grouped(observe.command.unwrap()),
            Command::Run(sts2_cli::control::Command::Observe)
        ));
    }

    /// The seconds the daemon is told to wait are the seconds it is spawned
    /// with, rather than the default it picks for itself.
    #[test]
    fn the_idle_timeout_survives_the_spawn() {
        let spawned = serving(std::time::Duration::from_secs(42), "v0.107.1").unwrap();
        let argv = std::iter::once(OsStr::new("sts2")).chain(spawned.get_args());

        let parsed = Cli::try_parse_from(argv).unwrap();

        assert!(matches!(
            grouped(parsed.command.unwrap()),
            Command::Held(Machinery::Serve {
                idle_timeout: 42,
                ..
            })
        ));
    }
}
