//! `sts2`: one game held open, one command per invocation.
//!
//! A command is a whole process, so a script or an agent can send one, read the
//! answer, and decide the next. A REPL does not offer that. Its caller has to
//! commit to every input before seeing any output.
//!
//! # What this front end knows about the game
//!
//! That it implements [`Construct`]. Nothing else.
//!
//! It does not know that the oracle boots in seconds, that something holds it
//! between two invocations, or that a unix socket is involved. Those
//! are facts about *that* implementation, and `sts2-headless` keeps them behind
//! [`Session`]. A simulator is a struct in this process with none of those
//! problems. Pointing this binary at one is meant to be the line below that
//! names the type.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sts2_cli::Readable;
use sts2_cli::cli::{Play, choose};
use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Answered, Applied, Construct, Step};
// Anonymously, because this crate's own `Play` is the clap subcommand group and
// the two would otherwise collide. Only its methods are wanted here.
use sts2_core::Play as _;
use sts2_headless::daemon::{self, DEFAULT_IDLE_TIMEOUT};
use sts2_headless::{Config, Session, recording};

/// The build this drives.
///
/// Named once. Every id follows from it through `ContentVersion`, so pointing
/// the tool at another build is a one-line change.
type V = V0_107_1;

/// The implementation this drives, named once for the same reason.
///
/// Everything below reaches it through [`Construct`], so the day there is a
/// simulator this line is what changes. `Session` is the oracle held open by a
/// daemon. `sts2_headless::Oracle` is one this process owns and kills on the
/// way out.
type Game = Session<V>;

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

#[derive(Debug)]
enum Command {
    Play(Play),
    Serve {
        detach: bool,
        idle_timeout: u64,
    },
    Choose {
        indices: Vec<sts2_cli::cli::Taking>,
    },
    Record {
        file: Option<PathBuf>,
    },
    Replay {
        file: PathBuf,
        steps: Option<usize>,
        record: Option<PathBuf>,
    },
    Map,
    Status,
    Stop,
}

fn grouped(top: Top) -> Command {
    match top {
        Top::Run { command } => match command {
            RunCommand::Start(args) => Command::Play(Play::Start(args)),
            RunCommand::Record { file } => Command::Record { file },
            RunCommand::Replay {
                file,
                steps,
                record,
            } => Command::Replay {
                file,
                steps,
                record,
            },
            RunCommand::Status => Command::Status,
            RunCommand::Stop => Command::Stop,
        },
        Top::Show { command } => match command {
            ShowCommand::State => Command::Play(Play::Observe),
            ShowCommand::Map => Command::Map,
        },
        Top::Act { command } => match command {
            ActCommand::Choose { indices } => Command::Choose { indices },
            ActCommand::Travel { col, row } => Command::Play(Play::Travel { col, row }),
            ActCommand::EnterRoom { room } => Command::Play(Play::EnterRoom { room }),
            ActCommand::PlayCard { index, target } => {
                Command::Play(Play::PlayCard { index, target })
            }
            ActCommand::EndTurn => Command::Play(Play::EndTurn),
            ActCommand::UsePotion { index, target } => {
                Command::Play(Play::UsePotion { index, target })
            }
            ActCommand::Proceed => Command::Play(Play::Proceed),
            ActCommand::ChooseOption { index } => Command::Play(Play::ChooseOption { index }),
            ActCommand::Buy { index } => Command::Play(Play::Buy { index }),
            ActCommand::Answer(answer) => Command::Play(Play::Answer(answer)),
        },
        Top::Setup { command } => Command::Play(match command {
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
            } => Command::Serve {
                detach,
                idle_timeout,
            },
        },
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let show = Show { rng: cli.rng };

    let Some(top) = cli.command else {
        return play(show);
    };

    let command = grouped(top);

    match command {
        Command::Serve {
            detach,
            idle_timeout,
        } => {
            let idle = Duration::from_secs(idle_timeout);
            if detach {
                daemon::spawn(serving(idle)?)?;
                println!("serving at {}", daemon::socket_path().display());
                return Ok(());
            }
            Ok(daemon::serve::<V>(game_config()?, idle)?)
        }

        Command::Choose { indices } => {
            let mut game = Game::new();

            // One extra look before acting, because which mechanism the game is
            // asking through is its state rather than the caller's intent.
            // Observing advances nothing, which is what makes looking safe.
            let looked = look(&mut game)?;
            let awaiting = choose::awaiting(&looked)?;
            let decision = choose::command::<V>(&awaiting, &indices)?;

            eprintln!("({})", choose::describe(&awaiting));
            let seen = send(&mut game, &decision.into())?;
            report(&game, seen, show)
        }

        Command::Record { file } => match Game::new().record(file, false)? {
            sts2_headless::Recorded { path: Some(p), .. } => {
                println!("recording to {}", p.display());
                Ok(())
            }
            sts2_headless::Recorded { steps, .. } => {
                println!("stopped recording after {steps} step(s)");
                Ok(())
            }
        },

        Command::Replay {
            file,
            steps,
            record,
        } => replay(&file, steps, record, show),

        Command::Map => {
            let seen = look(&mut Game::new())?;
            let map = sts2_cli::render::map::drawn(&seen.observation.map)
                .context("this observation carried no map")?;
            print!("{map}");
            Ok(())
        }

        Command::Status => {
            let status = Game::new().status()?;
            println!(
                "running  pid {}\ngame     {}\nidle     {}s of {}s",
                status.pid,
                status.game,
                status.idle.as_secs(),
                status.idle_timeout.as_secs()
            );
            if let Some(path) = status.recording {
                println!("record   {}", path.display());
            }
            Ok(())
        }

        Command::Stop => {
            if !daemon::running() {
                println!("nothing running");
                return Ok(());
            }
            Game::new().stop()?;
            println!("stopped");
            Ok(())
        }

        Command::Play(play) => {
            let Some(step) = play.step::<V>()? else {
                // `observe` reads as a command and is not a step in the run.
                let mut game = Game::new();
                let seen = look(&mut game)?;
                return report(&game, seen, show);
            };

            let mut game = Game::new();

            // Only `start` may bring a game up. Any other command against no
            // game is a mistake worth naming, not a twenty-second pause
            // followed by a refusal.
            if let Play::Start(args) = &play {
                if !daemon::running() {
                    daemon::spawn(serving(DEFAULT_IDLE_TIMEOUT)?)?;
                }
                // Before the `start` itself, so the recording opens with it. A
                // recording that does not begin at a `start` is not a run.
                if let Some(path) = args.recording()? {
                    begin_recording(&game, path, false)?;
                }
            }

            let seen = send(&mut game, &step)?;
            report(&game, seen, show)
        }
    }
}

/// Opens the game on the screen.
///
/// The daemon is not started for this. A run is begun with `sts2 run start`,
/// which is one line and says what it does. Bringing the game up on a bare
/// `sts2` means twenty seconds of nothing, followed by a screen with no run in
/// it. It also gives the player no way to say which seed they wanted.
fn play(show: Show) -> Result<()> {
    // The daemon comes up; the *game* does not. Launching the game is what
    // costs twenty seconds, and the daemon only does that when a command needs
    // one — so a bare `sts2` opens on the setup screen straight away and the
    // game boots while the player is picking a character.
    let fresh = !daemon::running();
    if fresh {
        daemon::spawn(serving(DEFAULT_IDLE_TIMEOUT)?)?;
    }

    let game = Game::new();
    let recording = game.status().ok().and_then(|status| status.recording);
    sts2_cli::tui::run::<V, _>(
        game,
        sts2_cli::tui::Options {
            recording,
            rng: show.rng,
            fresh,
        },
    )
}

/// Sends one step and returns what any implementation says, refusals as errors.
///
/// Uses only [`Construct`]. A refusal is a trait value because the game answered,
/// but becomes an error here because a shell needs a non-zero exit status.
///
/// `send` is the quiet path: a replay step, or the look `choose` takes before
/// it acts. A warning is worth saying once, and [`report`] says it.
fn send(game: &mut Game, step: &Step<V>) -> Result<Applied<V>> {
    answering(game.step(step)?)
}

/// Asks where the run is standing, without moving it.
///
/// What a front end does when it has nothing in hand, which is every time. This
/// is one process per command, so the last answer went out with the last
/// process.
fn look(game: &mut Game) -> Result<Applied<V>> {
    answering(game.observe()?)
}

/// A refusal, turned into an error.
///
/// It is the game answering rather than this failing, which is why it is a
/// value at the trait. But a shell wants a non-zero exit status for a move that
/// did not happen, and that is what an `Err` is here.
fn answering(answered: Answered<V>) -> Result<Applied<V>> {
    // Rendered here rather than through `Display`: `sts2-core` carries the
    // refusal as data and does not name a card, which is this crate's job.
    answered.map_err(|refused| anyhow::anyhow!("{}", sts2_cli::render::refusal::refusal(&refused)))
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

/// Everything the oracle adds to what a simulator says.
///
/// The rule line and the game's log are the protocol's and not the run's. So
/// they are handed to the renderer rather than reached for by it. A front end
/// driving something else prints the same page without them.
fn readable(game: &Game, applied: &Applied<V>, show: Show) -> String {
    let mut view = Readable::new(applied).logging(game.logged());
    if let Some((seq, cmd, digest)) = game.reported().and_then(envelope) {
        view = view.in_reply(seq, cmd, digest);
    }
    if show.rng {
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

/// Prints what one step produced, and anything that went wrong beside it.
///
/// The warning is the session failing rather than the game. A recording that
/// stopped means the move happened and the file did not. Saying nothing about
/// that is how a run gets played out under the belief it was being kept.
fn report(game: &Game, applied: Applied<V>, show: Show) -> Result<()> {
    if let Some(warning) = game.warned() {
        eprintln!("(warning: {warning})");
    }
    println!("{}", readable(game, &applied, show));
    Ok(())
}

/// The command that re-enters this binary as the daemon.
///
/// Built here because *which binary serves* is this front end's fact, not
/// `sts2-headless`'s. The crate owns the socket and the loop, and this owns the
/// knowledge that it has a `serve` subcommand to come back through.
fn serving(idle: Duration) -> Result<std::process::Command> {
    let exe = std::env::current_exe().context("finding this binary")?;
    let mut command = std::process::Command::new(exe);
    command
        .arg("serve")
        .arg("--idle-timeout")
        .arg(idle.as_secs().to_string());
    Ok(command)
}

/// Tells the session where to write the run, and fails loudly if it cannot.
///
/// Loudly because the alternative is a run played to the end under the belief
/// that it was being kept.
fn begin_recording(game: &Game, path: PathBuf, append: bool) -> Result<()> {
    game.record(Some(path.clone()), append)?;

    let verb = if append {
        "still recording"
    } else {
        "recording"
    };
    eprintln!("({verb} to {})", path.display());
    Ok(())
}

/// Sends a recorded run's steps into a fresh game.
///
/// Replays the run and prints only the final state. The caller wants to
/// stand where the recording ended.
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

fn replay(
    file: &PathBuf,
    stop_after: Option<usize>,
    record: Option<PathBuf>,
    show: Show,
) -> Result<()> {
    let opened =
        std::fs::File::open(file).with_context(|| format!("opening {}", file.display()))?;
    let recording = recording::read::<V>(BufReader::new(opened))
        .with_context(|| format!("reading {}", file.display()))?;

    let steps = match stop_after {
        Some(n) if n < recording.len() => &recording.steps()[..n],
        _ => recording.steps(),
    };

    let Some(first) = steps.first() else {
        bail!("{} has no steps in it", file.display());
    };
    if !matches!(first, Step::Decision(sts2_core::Decision::Start { .. })) {
        // A recording begun mid-run replays into a different run, silently: the
        // seed is unset, so every shuffle differs from the first step on.
        bail!(
            "{} does not begin with a `start`, so it is a fragment rather than a run",
            file.display()
        );
    }

    // A replay is a new run, so it needs a game with nothing in it. Stopping
    // rather than reusing: a daemon holding a half-played run would apply these
    // steps on top of it.
    if daemon::running() {
        Game::new().stop()?;
        while daemon::running() {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    daemon::spawn(serving(DEFAULT_IDLE_TIMEOUT)?)?;
    let mut game = Game::new();

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
        begin_recording(&game, path.clone(), false)?;
    }

    let total = steps.len();
    let mut last = None;
    for (at, step) in steps.iter().enumerate() {
        eprint!("\rreplaying {}/{total}", at + 1);
        // Line-buffered output would hold the counter until the newline below,
        // so a long replay would show nothing until it had finished.
        let _ = std::io::Write::flush(&mut std::io::stderr());
        // Through `send` rather than `report`, so a step the game refuses stops
        // the replay at the step that broke rather than at whatever came after.
        last = Some(send(&mut game, step).with_context(|| format!("step {} of {total}", at + 1))?);
    }
    eprintln!();

    // The source file is resumed *after*, because it already holds every step
    // just sent. Appending rather than rewriting is also what keeps a replay
    // that failed halfway from truncating the very run somebody opened it to
    // look at.
    //
    // `--steps` is a branch: writing into the source from there would drop the
    // steps that were not replayed. So a branch records only where it is told
    // to, and says it is not recording otherwise — silently stopping is how a
    // session gets played out under the belief it was being kept.
    if carry_on {
        begin_recording(&game, file.clone(), true)?;
    } else if record.is_none() {
        eprintln!("(not recording — pass --record FILE to play on into one)");
    }

    match last {
        Some(response) => {
            println!("{}", readable(&game, &response, show));
            Ok(())
        }
        None => Ok(()),
    }
}

fn game_config() -> Result<Config> {
    let dir =
        sts2_headless::game_dir_from_env().context("set STS2_GAME_DIR to the game's directory")?;

    Ok(Config::new(dir))
}
