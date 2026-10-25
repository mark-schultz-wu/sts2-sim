//! One co-op run the game plays at random, one game for each seat, for
//! `cargo xtask coop`.
//!
//! ```text
//! coop_run <seed> <character>,<character>... --homes <dir> [--steps <n>]
//! ```
//!
//! Seat 0 hosts, and each seat is a game of its own over ENet. [`Coop`] picks
//! the seat that acts next, and the seat picks its move from its own legal
//! list, as `record_run` does. Each seat's game gets a profile of its own
//! under `--homes`.
//!
//! The build is `STS2_GAME_VERSION`'s, the setting that chose
//! `STS2_GAME_DIR`. Only v0.111.0 plays co-op: v0.107.1 answers the other
//! seats' card choices itself.
//!
//! It prints one line for each move, and a last line for how the run ended.
//! It exits 0 when the run ends. It exits 1 when a move is refused, a game
//! logs an error, the seats stall, or a game fails.

use std::net::{Ipv4Addr, UdpSocket};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use sts2_content_wire::WireContentVersion;
use sts2_core::{Coop, Lobby, Seed, Turn};
use sts2_golden::random::{Pick, making_room};
use sts2_headless::{Config, GameSeat, SeatHost, game_dir_from_env};

/// Moves a run takes before this gives up on it.
const STEPS: usize = 20_000;

/// Steps in a row with no seat to act before the run counts as stalled.
///
/// Each step looks at every seat, and looking waits until the seat's game
/// stands still. So a stall that lasts is the seats waiting on each other.
const STALLS: usize = 20;

/// Play one co-op run on the game at random.
#[derive(Parser)]
struct Cli {
    /// The seed, such as `C000000001`.
    #[arg(value_parser = seed)]
    seed: Seed,
    /// Each seat's character, by id, in seat order, such as
    /// `IRONCLAD,SILENT`.
    #[arg(value_delimiter = ',', required = true)]
    characters: Vec<String>,
    /// A directory for the seats' profiles, one under it for each seat.
    #[arg(long, value_name = "DIR")]
    homes: PathBuf,
    /// Moves to play before giving up.
    #[arg(long, default_value_t = STEPS)]
    steps: usize,
}

fn seed(text: &str) -> Result<Seed, String> {
    Seed::new(text).map_err(|e| e.to_string())
}

#[derive(Debug, thiserror::Error)]
enum Error<V: WireContentVersion> {
    #[error("no game: set STS2_GAME_DIR")]
    NoGame,
    #[error("{0:?} is no character this build has")]
    Character(String),
    #[error("finding a port to host on: {0}")]
    Port(std::io::Error),
    #[error(transparent)]
    Game(#[from] sts2_headless::Error),
    #[error("seat {seat} was refused {what}: {refusal}")]
    Refused {
        seat: u8,
        what: String,
        refusal: sts2_core::Refusal<V>,
    },
    #[error("seat {seat} logged at step {step}: {errors:?}")]
    Logged {
        seat: u8,
        step: usize,
        errors: Vec<String>,
    },
    #[error("no seat has a move at step {step}, and the run goes on")]
    Stalled { step: usize },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // One arm per build `sts2-content` compiles.
    let version = std::env::var("STS2_GAME_VERSION").unwrap_or_else(|_| "v0.107.1".to_string());
    let went = match version.as_str() {
        "v0.107.1" => play::<sts2_content::v0_107_1::V0_107_1>(cli).map_err(|e| e.to_string()),
        "v0.111.0" => play::<sts2_content::v0_111_0::V0_111_0>(cli).map_err(|e| e.to_string()),
        other => Err(format!("no build {other:?}; try v0.111.0")),
    };
    match went {
        Ok(ending) => {
            println!("{ending}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A free UDP port on this machine, for the host to listen on.
fn free_port() -> std::io::Result<u16> {
    UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?
        .local_addr()
        .map(|at| at.port())
}

/// Plays the run, and says how it ended.
fn play<V: WireContentVersion>(cli: Cli) -> Result<String, Error<V>> {
    let Cli {
        seed,
        characters,
        homes,
        steps,
    } = cli;
    let characters = characters
        .iter()
        .map(|id| {
            serde_json::from_value(serde_json::Value::String(id.to_uppercase()))
                .map_err(|_| Error::Character(id.clone()))
        })
        .collect::<Result<Vec<V::Character>, _>>()?;
    let game_dir = game_dir_from_env().ok_or(Error::NoGame)?;
    let port = free_port().map_err(Error::Port)?;
    let hosts = (0..characters.len())
        .map(|at| SeatHost {
            config: Config::new(&game_dir).data_home(homes.join(format!("seat{at}"))),
            port,
        })
        .collect();
    let mut pick = Pick::for_seed(seed.as_str());
    let lobby = Lobby { seed, characters };
    let mut coop = Coop::<V, GameSeat<V>>::begin(hosts, &lobby)?;

    let mut stalls = 0;
    for step in 0..steps {
        let turn = coop.step(|_, seated, moves| {
            let room = making_room(
                seated.snapshot.asking.as_ref(),
                seated.snapshot.observation.potions.len(),
                seated.snapshot.observation.potion_slots,
            );
            pick.choose(moves, room)
        })?;
        // First, so the log names the move a failure below comes after.
        if let Turn::Played { seat, decision, .. } = &turn {
            println!("{step} seat {}: {decision:?}", seat.0);
        }
        // Every seat, because a game can log while another seat's move plays.
        for (at, seat) in coop.seats_mut().iter_mut().enumerate() {
            let errors = seat.take_logged();
            if !errors.is_empty() {
                return Err(Error::Logged {
                    seat: u8::try_from(at).expect("at most 256 seats"),
                    step,
                    errors,
                });
            }
        }
        match turn {
            Turn::Played {
                seat,
                decision,
                answered,
            } => {
                stalls = 0;
                if let Err(refusal) = *answered {
                    return Err(Error::Refused {
                        seat: seat.0,
                        what: format!("{decision:?} at step {step}"),
                        refusal,
                    });
                }
            }
            Turn::Stalled(_) => {
                stalls += 1;
                if stalls == STALLS {
                    return Err(Error::Stalled { step });
                }
            }
            Turn::Over(seated) => {
                let seat = &seated[0].snapshot.observation;
                return Ok(format!(
                    "over at step {step}, floor {}, {}",
                    seat.floor,
                    match seat.victory {
                        Some(true) => "won",
                        _ => "lost",
                    }
                ));
            }
        }
    }
    Ok(format!("capped at {steps} steps"))
}
