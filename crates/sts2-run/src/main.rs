//! `sts2-run`: one run, played at a table by a player, to a verdict, or one
//! recorded run replayed. `following.rs` says how `follow` replays.
//!
//! ```text
//! sts2-run follow <file> [index] [--at <step> | --scenario | --diff <dir>]
//! sts2-run play <seed> <characters> [--build v0.107.1] [--table sim|game]
//!          [--player random|uniform] [--carried] [--steps N]
//!          [--record FILE] [--hunt] [--home DIR] [--moves]
//!          [--ascension ID] [--modifiers ID,...]
//! ```
//!
//! `<characters>` is one character for a solo run, or several joined by
//! commas for a run of several players, one seat each. The last line on
//! stdout is the run's [`Verdict`]. `docs/RUNNERS.md` says what each layer
//! does.

use std::net::{Ipv4Addr, UdpSocket};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use sts2_core::{Player as Lobbyist, RunTable, Seed, Start, Verdict};
use sts2_headless::{Config, LobbyTable, OracleTable, SeatHost, game_dir_from_env};
use sts2_run::{
    Carried, Fault, Hunted, LogGuard, MovesLog, Random, Recorded, Uniform, ids::read_id,
};
use sts2_sim::SimTable;

/// Moves a run takes before this gives up on it.
const STEPS: usize = 20_000;

mod following;

/// Play a run, or replay a recorded one.
#[derive(Parser)]
enum Cli {
    /// Play one run.
    Play(Play),
    /// Replay a recorded run.
    Follow(following::Args),
}

/// Play one run.
#[derive(clap::Args)]
struct Play {
    /// The seed, such as `C000000042`.
    #[arg(value_parser = Seed::new)]
    seed: Seed,
    /// The characters by id, one for each seat, such as `IRONCLAD` or
    /// `IRONCLAD,SILENT`.
    #[arg(value_delimiter = ',', required = true)]
    characters: Vec<String>,
    /// The build to play, as the game names its version.
    #[arg(long, default_value = "v0.107.1")]
    build: String,
    /// What holds the run.
    #[arg(long, value_enum, default_value_t = Table::Sim)]
    table: Table,
    /// What picks every seat's moves.
    #[arg(long, value_enum, default_value_t = Pick::Random)]
    player: Pick,
    /// Every player carries 25 Gorgets and a Mercury Hourglass.
    #[arg(long)]
    carried: bool,
    /// The moves played before the run is called capped.
    #[arg(long, default_value_t = STEPS)]
    steps: usize,
    /// Write the run, as a file of recorded runs, here.
    #[arg(long)]
    record: Option<PathBuf>,
    /// Play the simulator beside the game, and stop where the two part.
    #[arg(long)]
    hunt: bool,
    /// The game's profile directory. A run of several players gives each
    /// seat `seat<n>` in it. A hunt writes where it parted here too.
    #[arg(long)]
    home: Option<PathBuf>,
    /// Print each move as it is played.
    #[arg(long)]
    moves: bool,
    /// The ascension level, by the id the game spells it with.
    #[arg(long, default_value = "None")]
    ascension: String,
    /// The modifiers the run starts with, by the ids the game spells them
    /// with. Character Cards names its character after a colon, as
    /// `CHARACTER_CARDS:DEFECT`.
    #[arg(long, value_delimiter = ',')]
    modifiers: Vec<String>,
}

/// What holds the run.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Table {
    /// The simulator, one machine's view for each seat.
    Sim,
    /// The game, one process for each seat. `STS2_GAME_DIR` names it.
    Game,
}

/// What picks the moves.
#[derive(Clone, Copy, ValueEnum)]
enum Pick {
    /// At random, as the census and the recorder pick.
    Random,
    /// Every legal move alike, Cancel among them, as a fuzzer picks.
    Uniform,
}

/// The player the run was given.
enum Picking {
    Random(Random),
    Uniform(Uniform),
}

impl<V: sts2_core::ContentVersion> sts2_run::Player<V> for Picking {
    fn choose<T: RunTable<V>>(
        &mut self,
        seat: sts2_core::SeatIndex,
        table: &mut T,
        moves: &[sts2_core::Decision<V>],
    ) -> Result<Option<sts2_core::Decision<V>>, T::Error> {
        match self {
            Picking::Random(player) => player.choose(seat, table, moves),
            Picking::Uniform(player) => player.choose(seat, table, moves),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("no build {0:?}; try v0.107.1 or v0.111.0")]
    Build(String),
    #[error("{id:?} names nothing: {source}")]
    Id {
        id: String,
        source: serde_json::Error,
    },
    #[error("STS2_GAME_DIR is unset")]
    NoGame,
    #[error("a hunt plays the simulator beside the game, so it needs --table game")]
    HuntWithoutGame,
    #[error("finding a free port for the lobby: {0}")]
    Port(std::io::Error),
    #[error("the profile directory: {0}")]
    Home(std::io::Error),
}

fn main() -> ExitCode {
    sts2_run::keep_panics();
    let cli = match Cli::parse() {
        Cli::Play(play) => play,
        Cli::Follow(args) => {
            return match following::run(args) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            };
        }
    };
    // One arm per build `sts2-content` compiles.
    let played = match cli.build.as_str() {
        "v0.107.1" => run::<sts2_content::v0_107_1::V0_107_1>(cli),
        "v0.111.0" => run::<sts2_content::v0_111_0::V0_111_0>(cli),
        other => Err(Error::Build(other.to_owned())),
    };
    match played {
        Ok(verdict) => {
            println!("{verdict}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Content by the id the game spells it with.
fn id<T: serde::de::DeserializeOwned>(id: &str) -> Result<T, Error> {
    read_id(id).map_err(|source| Error::Id {
        id: id.to_owned(),
        source,
    })
}

/// The run `cli` names, played to its verdict.
fn run<V>(cli: Play) -> Result<Verdict, Error>
where
    V: sts2_content::Build + sts2_content_wire::WireContentVersion,
    sts2_core::Seated<V>: Send,
    sts2_core::Answered<V>: Send,
    sts2_core::Setup<V>: Sync,
{
    let Play {
        seed,
        characters,
        build: _,
        table,
        player,
        carried,
        steps,
        record,
        hunt,
        home,
        moves,
        ascension,
        modifiers,
    } = cli;
    let characters = characters
        .iter()
        .map(|character| id::<V::Character>(&character.to_uppercase()))
        .collect::<Result<Vec<_>, _>>()?;
    // A solo run's profile has won as often as its seed says, for the
    // Architect's last line. A co-op lobby's players have won nothing.
    let wins = match characters.len() {
        1 => sts2_run::random::wins(seed.as_str()),
        _ => 0,
    };
    let start = Start {
        players: characters
            .into_iter()
            .map(|character| Lobbyist { character, wins })
            .collect(),
        ascension: id(&ascension)?,
        modifiers: modifiers
            .iter()
            .map(|text| {
                sts2_run::ids::read_modifier(text).map_err(|source| Error::Id {
                    id: text.clone(),
                    source,
                })
            })
            .collect::<Result<_, _>>()?,
        seed,
    };
    let mut player = match player {
        Pick::Random => Picking::Random(Random::for_seed(start.seed.as_str())),
        Pick::Uniform => Picking::Uniform(Uniform::for_seed(start.seed.as_str())),
    };

    if table == Table::Sim {
        if hunt {
            return Err(Error::HuntWithoutGame);
        }
        return Ok(match (carried, record) {
            (false, None) => at::<V, SimTable<V>>((), &start, &mut player, steps),
            (true, None) => at::<V, Carried<SimTable<V>>>((), &start, &mut player, steps),
            (false, out) => {
                at::<V, Recorded<V, SimTable<V>>>((out, ()), &start, &mut player, steps)
            }
            (true, out) => {
                at::<V, Carried<Recorded<V, SimTable<V>>>>((out, ()), &start, &mut player, steps)
            }
        });
    }

    let game_dir = game_dir_from_env().ok_or(Error::NoGame)?;
    // Absolute, because a game given a relative profile directory falls back
    // to its default one, and two seats then race on one save.
    let home = home
        .map(std::path::absolute)
        .transpose()
        .map_err(Error::Home)?;
    let dir = home.clone().unwrap_or_else(|| PathBuf::from("."));
    let hunted = hunt.then(|| dir.clone());
    if start.players.len() == 1 {
        let mut config = Config::new(&game_dir);
        config.data_home = home;
        let host = (moves, (hunted, (record, config)));
        type Game<V> = MovesLog<Hunted<V, Recorded<V, LogGuard<OracleTable<V>>>>>;
        return Ok(match carried {
            true => at::<V, Carried<Game<V>>>(host, &start, &mut player, steps),
            false => at::<V, Game<V>>(host, &start, &mut player, steps),
        });
    }
    let seats = lobby(&game_dir, &dir, start.players.len())?;
    let host = (moves, (hunted, (record, seats)));
    type Lobby<V> = MovesLog<Hunted<V, Recorded<V, LogGuard<LobbyTable<V>>>>>;
    Ok(match carried {
        true => at::<V, Carried<Lobby<V>>>(host, &start, &mut player, steps),
        false => at::<V, Lobby<V>>(host, &start, &mut player, steps),
    })
}

/// A host for each of `players` seats of a lobby, on the game in `game`,
/// each with a profile `seat<n>` under `dir`.
///
/// `STS2_HEADLESS_TRACE` reaches every seat's game, and each traces to a
/// file of its own beside its profile, as `cargo xtask game-trace --coop`
/// reads them.
fn lobby(
    game: &std::path::Path,
    dir: &std::path::Path,
    players: usize,
) -> Result<Vec<SeatHost>, Error> {
    let tracing = std::env::var_os("STS2_HEADLESS_TRACE").is_some();
    let port = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
        .and_then(|socket| socket.local_addr())
        .map_err(Error::Port)?
        .port();
    Ok((0..players)
        .map(|at| {
            let config = Config::new(game).data_home(dir.join(format!("seat{at}")));
            SeatHost {
                config: match tracing {
                    true => config.trace_file(dir.join(format!("seat{at}.trace"))),
                    false => config,
                },
                port,
            }
        })
        .collect())
}

/// The run `start` names at a table of `T`, to its verdict.
fn at<V, T>(host: T::Host, start: &Start<V>, player: &mut Picking, steps: usize) -> Verdict
where
    V: sts2_core::ContentVersion,
    T: RunTable<V>,
    Fault: From<T::Error>,
{
    sts2_run::run::<V, T, Picking>(host, start, player, steps)
}
