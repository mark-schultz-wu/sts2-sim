//! Random legal play until the run or game ends.
//!
//! Random decisions reach states rarely constructed by hand, where oracle bugs
//! tend to hide. A refusal, a logged game error, or game death stops the run.
//! Every run is recorded so a finding is reproducible.
//!
//! `Gorget` is stacked by default because unassisted random runs die before
//! reaching interesting states. The fuzzer owns its game process and never
//! evicts a run from the interactive daemon.

use clap::Parser;
use std::fs::File;
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use sts2_cli::cli::{id, moves};
use sts2_cli::coverage::Coverage;
use sts2_core::Seed;
use sts2_core::{Construct, Decision, Gift, Play, Refusal, Setup, Snapshot, Start, Step};
use sts2_headless::{Config, Oracle, recording};

#[derive(Debug, Parser)]
#[command(
    name = "sts2-fuzz",
    about = "Play a run at random until something breaks."
)]
struct Cli {
    /// The run's seed. One is made up when this is absent.
    #[arg(long)]
    seed: Option<String>,
    /// Which pinned build to fuzz, as its lock under `mod/games/` names it.
    ///
    /// It has to be the build `STS2_GAME_DIR` points at. The tables decide how
    /// an observation is read, and reading one build's game with another's is
    /// how a fuzzer reports faults that are its own.
    ///
    /// Defaults to `STS2_GAME_VERSION`, which is what `.envrc` exports and what
    /// picks the game directory, so the two cannot drift apart by default.
    #[arg(long)]
    game_version: Option<String>,
    /// A character id, such as `IRONCLAD`.
    #[arg(long, default_value = "IRONCLAD")]
    character: String,
    /// An ascension id, such as `Poverty`. `None` is the bottom of the ladder.
    #[arg(long, default_value = "None")]
    ascension: String,
    /// The relic to lean on, granted `--stack` times before play starts.
    #[arg(long, default_value = "GORGET")]
    relic: String,
    /// How many of it. Survivability is the point: a run that dies on floor
    /// three tests floor three.
    #[arg(long, default_value_t = 100)]
    stack: usize,
    /// Give up after this many moves, so a loop cannot run forever.
    #[arg(long, default_value_t = 5000)]
    steps: usize,
    /// Where to write the run. Defaults to a name of its own beside the others.
    #[arg(long, value_name = "FILE")]
    record: Option<PathBuf>,
    /// Let the game's own log through to this process's output.
    ///
    /// For the findings the recording cannot explain. "The game closed the
    /// connection" says the process died and nothing about why. The exception
    /// it printed on the way out does, and without this it goes to /dev/null.
    #[arg(long)]
    game_output: bool,
    /// A directory for the game to keep its profile in, apart from the
    /// user's, so several fuzzers can run at once without racing on saves.
    #[arg(long, value_name = "DIR")]
    data_home: Option<PathBuf>,
}

/// Why a fuzz run stopped short of an ending.
#[derive(Debug, thiserror::Error)]
enum Error<V: sts2_content_wire::WireContentVersion> {
    #[error(transparent)]
    Seed(#[from] sts2_core::seed::Error),
    #[error(transparent)]
    Cli(#[from] sts2_cli::cli::Error),
    #[error("reading the clock: {0}")]
    Clock(#[from] std::time::SystemTimeError),
    #[error("{what} {}: {source}", path.display())]
    Io {
        what: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    #[error("serialising coverage: {0}")]
    Coverage(#[from] serde_json::Error),
    #[error("set STS2_GAME_DIR to the game directory (`just fuzz` does this)")]
    NoGameDir,
    #[error("launching the game: {0}")]
    Launch(sts2_headless::Error),
    #[error(transparent)]
    Sent(#[from] Sent<V>),
    #[error("nothing is legal and the run is not over, after {taken} move(s); {}", path.display())]
    NothingLegal { taken: usize, path: PathBuf },
    #[error("replay {}: move {taken}: {chosen:?}: {source}", path.display())]
    Move {
        path: PathBuf,
        taken: usize,
        /// Boxed, as a selection makes a decision too large to return.
        chosen: Box<Decision<V>>,
        source: Sent<V>,
    },
}

/// Why a move sent is a finding.
#[derive(Debug, thiserror::Error)]
enum Sent<V: sts2_content_wire::WireContentVersion> {
    #[error("writing the recording: {0}")]
    Tape(#[from] io::Error),
    #[error("the game stopped answering: {0}")]
    Silent(#[from] sts2_headless::Error),
    /// Not "the move was wrong". The move came out of the legal set, so a
    /// refusal means that set and the game disagree, which is the bug.
    #[error("the game refused a move it offered: {0}")]
    Refused(Refusal<V>),
    #[error("the game logged {} error(s): {}", .0.len(), .0.join(" · "))]
    Logged(Vec<String>),
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // One arm per build `sts2-content` compiles. The whole fuzzer is generic
    // over the version, so this is the only place that names one, and adding a
    // build is adding a line.
    let version = cli
        .game_version
        .clone()
        .or_else(|| std::env::var("STS2_GAME_VERSION").ok())
        .unwrap_or_else(|| "v0.107.1".to_string());

    match version.as_str() {
        "v0.107.1" => exit(fuzz::<sts2_content::v0_107_1::V0_107_1>(cli)),
        "v0.111.0" => exit(fuzz::<sts2_content::v0_111_0::V0_111_0>(cli)),
        other => {
            eprintln!("no tables for game version {other:?}; try v0.107.1 or v0.111.0");
            ExitCode::FAILURE
        }
    }
}

/// The exit status for how a run went, with the error on stderr.
fn exit(fuzzed: Result<(), impl std::fmt::Display>) -> ExitCode {
    match fuzzed {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn fuzz<V: sts2_content_wire::WireContentVersion>(cli: Cli) -> Result<(), Error<V>> {
    let seed = match &cli.seed {
        Some(typed) => Seed::fold(typed)?,
        None => made_up_seed()?,
    };
    let character: V::Character = id("character", &cli.character)?;
    let relic: V::Relic = id("relic", &cli.relic)?;
    let ascension: V::AscensionLevel = id("ascension level", &cli.ascension)?;

    let path = match cli.record.clone() {
        Some(path) => path,
        None => {
            let dir = sts2_cli::runs::runs_dir();
            std::fs::create_dir_all(&dir).map_err(|source| Error::Io {
                what: "making",
                path: dir.clone(),
                source,
            })?;
            dir.join(format!(
                "fuzz-{}-{}.jsonl",
                seed.as_str(),
                cli.character.to_uppercase()
            ))
        }
    };

    let mut tape = BufWriter::new(File::create(&path).map_err(|source| Error::Io {
        what: "creating",
        path: path.clone(),
        source,
    })?);
    recording::open::<V>(&mut tape).map_err(Sent::Tape)?;
    println!("seed {} · recording to {}", seed.as_str(), path.display());

    let dir = sts2_headless::game_dir_from_env().ok_or(Error::NoGameDir)?;
    let mut config = Config::new(dir);
    config.inherit_game_output = cli.game_output;
    config.data_home = cli.data_home.clone();
    let start = Start {
        seed: seed.clone(),
        character,
        ascension,
        modifiers: vec![],
        wins: 0,
    };
    // The run is begun with the game, because a simulator or an oracle without
    // one is not something to fuzz.
    let mut game = Oracle::<V>::begin(config, &start).map_err(Error::Launch)?;

    // The seed decides the run. This decides which of the legal moves gets
    // taken. Derived from it so a session is reproducible from its name alone,
    // even before the recording is read back.
    //
    // One build's seeding, whichever build is being fuzzed. These dice are the
    // fuzzer's own and never the game's, so fidelity is not the point, and the
    // same name has to pick the same moves on both builds for two runs to be
    // worth putting side by side.
    let mut dice = Dice::from(&seed);

    let mut cover = Coverage::default();
    cover.started::<V>(character);

    // The start is the recording's first line, and it happened above.
    recording::began(&mut tape, &start).map_err(Sent::Tape)?;
    let mut seen = game.observe().map_err(Sent::Silent)?;
    cover.saw(&seen);

    for _ in 0..cli.stack {
        seen = send(
            &mut game,
            &mut tape,
            &Setup::Grant(Gift::Relic(relic)).into(),
        )?;
    }
    cover.saw(&seen);

    for taken in 1..=cli.steps {
        if let Some(over) = finished(&seen) {
            println!("{over} after {taken} move(s) · {}", path.display());
            return keep(&path, &cover);
        }

        let legal = moves::legal::<V>(&seen);
        if legal.is_empty() {
            keep(&path, &cover)?;
            return Err(Error::NothingLegal { taken, path });
        }

        let chosen = legal[dice.below(legal.len())];
        // Before the move is sent, because it is read against the state the
        // move was chosen in: afterwards the hand it indexes into is gone.
        cover.did::<V>(&chosen, &seen);

        seen = match send(&mut game, &mut tape, &chosen.into()) {
            Ok(seen) => seen,
            Err(source) => {
                keep(&path, &cover)?;
                // Beside the recording rather than only on the terminal. A
                // finding is a game exception with its whole stack in it, and
                // scrolled off the top of a batch it is unrecoverable: the run
                // that produced it takes minutes to replay and the message is
                // the only thing that says what to look at.
                //
                // With the state the move was chosen in. Not every finding
                // explains itself -- "the game closed the connection" says the
                // process died and nothing about what it was holding when it
                // did -- and this is the last state anything saw. Replaying a
                // thousand moves to get it back costs minutes; keeping it costs
                // a file.
                note(
                    &path,
                    &format!(
                        "move {taken}: {chosen:?}: {source}\n\nthe state it was chosen in:\n{:#?}",
                        seen.observation
                    ),
                );
                return Err(Error::Move {
                    path,
                    taken,
                    chosen: Box::new(chosen),
                    source,
                });
            }
        };

        cover.saw(&seen);

        if taken % 50 == 0 {
            println!("{taken} moves · {}", where_it_is(&seen));
        }
    }

    println!("{} moves and still going · {}", cli.steps, path.display());
    keep(&path, &cover)
}

/// Writes what the run reached, beside the run.
///
/// Every exit writes it, a finding included. A run that stopped on a bug still
/// covered everything it walked through on the way. That is most of what a
/// batch is measuring.
fn keep<V: sts2_content_wire::WireContentVersion>(
    recording: &Path,
    cover: &Coverage,
) -> Result<(), Error<V>> {
    let path = recording.with_extension("coverage.json");
    let json = serde_json::to_string(cover)?;
    std::fs::write(&path, json).map_err(|source| Error::Io {
        what: "writing",
        path,
        source,
    })
}

/// Writes the move down, sends it, and treats a refusal or a logged error as a
/// finding.
///
/// Written before it is sent, so a move that kills the game is in the file that
/// reproduces it. A move the game refuses is in there too, which is what makes
/// the file replay to the state the refusal was found in.
fn send<V: sts2_content_wire::WireContentVersion>(
    game: &mut Oracle<V>,
    tape: &mut BufWriter<File>,
    step: &Step<V>,
) -> Result<Snapshot<V>, Sent<V>> {
    recording::append(&mut *tape, step)?;

    // `Construct::step` reads the state on the way through, so every state a
    // fuzz run walks past is converted. That is what puts `Observation<V>` under
    // load: a key the mod adds, a category with no type, an intent shape nobody
    // wrote a test for — thousands of states an afternoon, each one refused
    // rather than half-read.
    let seen = game.step(step)?.map_err(Sent::Refused)?.snapshot();

    let errors = game.logged();
    if !errors.is_empty() {
        return Err(Sent::Logged(errors.to_vec()));
    }

    Ok(seen)
}

/// Writes a finding down beside the run that produced it.
///
/// Best-effort: a report that cannot be filed must not replace the finding it
/// was reporting, so a failure here is said once and otherwise ignored.
fn note(recording: &Path, what: &str) {
    let path = recording.with_extension("finding.txt");
    match std::fs::write(&path, what) {
        Ok(()) => println!("finding written to {}", path.display()),
        Err(e) => eprintln!("(could not write {}: {e})", path.display()),
    }
}

/// Why the run stopped, if it has.
///
/// `victory` is only meaningful once the run is over, because winning kills
/// every player too. The order of these two tests is the whole of it.
fn finished<V: sts2_content_wire::WireContentVersion>(
    snapshot: &Snapshot<V>,
) -> Option<&'static str> {
    if !snapshot.observation.run_over {
        return None;
    }
    Some(match snapshot.observation.victory {
        Some(true) => "won",
        _ => "died",
    })
}

/// Enough of the state to watch progress without printing the whole thing.
fn where_it_is<V: sts2_content_wire::WireContentVersion>(snapshot: &Snapshot<V>) -> String {
    let o = &snapshot.observation;
    format!(
        "floor {} · act {} · hp {}/{}",
        o.floor, o.act, o.hp, o.max_hp
    )
}

/// A seed nobody chose, from the clock.
fn made_up_seed<V: sts2_content_wire::WireContentVersion>() -> Result<Seed, Error<V>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos() as u64;

    let mut dice = Dice::from(now);
    let typed: String = (0..8)
        .map(|_| {
            Seed::ALPHABET
                .as_bytes()
                .get(dice.below(Seed::ALPHABET.len()))
                .copied()
                .unwrap_or(b'0') as char
        })
        .collect();

    Ok(Seed::new(&typed)?)
}

/// Which of the legal moves to take.
///
/// Its own xorshift rather than the game's `Rng`. This picks between the
/// oracle's moves and must never touch a stream the run is drawing from. It is
/// small enough that a dependency costs more than it saves.
struct Dice(u64);

impl From<u64> for Dice {
    fn from(seed: u64) -> Self {
        // Zero is the one state xorshift cannot leave.
        Self(seed | 1)
    }
}

impl From<&Seed> for Dice {
    /// Folded here rather than through the game's seeding, which this must not
    /// share. What is needed is only that the same name picks the same moves.
    fn from(seed: &Seed) -> Self {
        let mut state: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in seed.as_str().bytes() {
            state ^= u64::from(byte);
            state = state.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self::from(state)
    }
}

impl Dice {
    fn roll(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        match bound {
            0 => 0,
            _ => (self.roll() % bound as u64) as usize,
        }
    }
}
