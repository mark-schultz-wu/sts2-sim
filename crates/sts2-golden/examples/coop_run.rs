//! One co-op run the game plays at random, one game for each seat, for
//! `cargo xtask coop`.
//!
//! ```text
//! coop_run <seed> <character>,<character>... --homes <dir> [--steps <n>] [--out <file>]
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
//! logs an error, the seats stall, or a game fails. `--hunt` is [`Hunt`].

use std::net::{Ipv4Addr, UdpSocket};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use sts2_content_wire::WireContentVersion;
use sts2_core::{AscensionKinds as _, Coop, Player, Seat as _, Seed, Start, Turn};
use sts2_golden::random::{Pick, making_room};
use sts2_golden::recorded::{Ending, Recorded, Run, named, parted, position};
use sts2_golden::scenario::Stamp;
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
    /// Where to write the run, once it ends or reaches `--steps`. It is a
    /// file of recorded runs, as `record_run` writes a solo one.
    #[arg(long, value_name = "FILE")]
    out: Option<PathBuf>,
    /// Play the simulator beside the game, and stop where they part.
    #[arg(long)]
    hunt: bool,
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
    #[error("writing the run: {0}")]
    Write(std::io::Error),
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
fn play<V: WireContentVersion + sts2_content::Build>(cli: Cli) -> Result<String, Error<V>> {
    let Cli {
        seed,
        characters,
        homes,
        steps,
        out,
        hunt,
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
    let start = Start {
        seed,
        players: characters
            .into_iter()
            .map(|character| Player { character, wins: 0 })
            .collect(),
        ascension: V::AscensionLevel::NONE,
        modifiers: Vec::new(),
    };
    let mut coop = Coop::<V, GameSeat<V>>::begin(hosts, &start)?;
    let mut run = Run::of(&start, Ending::Capped);
    let mut hunt = match hunt {
        true => match Hunt::begin(&start) {
            Ok(hunt) => Some(hunt),
            Err(why) => return Ok(format!("panicked 0 as the run began: {why}")),
        },
        false => None,
    };

    let mut stalls = 0;
    for step in 0..steps {
        let mut verdict = None;
        let turn = coop.step(|me, seated, moves| {
            let room = making_room(
                seated.snapshot.asking.as_ref(),
                seated.snapshot.observation.potions.len(),
                seated.snapshot.observation.potion_slots,
            );
            let at = pick.choose(moves, room);
            if let Some(hunt) = hunt.as_mut() {
                verdict = hunt.compare(me, &seated.snapshot).map(|fields| {
                    format!(
                        "parted {step} seat {} after {} in {fields}",
                        me.0, hunt.after
                    )
                });
                hunt.after = named(&seated.snapshot, &moves[at]);
            }
            run.digests.push(position(&seated.snapshot));
            run.moves
                .push(u16::try_from(at).expect("a list of moves fits a u16"));
            run.seats.push(me);
            at
        })?;
        if let Some(verdict) = verdict {
            run.ending = Ending::Parted;
            return finish(&coop, run, out.as_deref(), verdict);
        }
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
                if let Some(hunt) = hunt.as_mut()
                    && let Err(why) = hunt.play(seat, &decision)
                {
                    run.ending = Ending::Parted;
                    let (kind, me, message) = why;
                    let said = format!(
                        "{kind} {step} seat {}: {message}, after seat {}'s {decision:?}",
                        me.0, seat.0
                    );
                    return finish(&coop, run, out.as_deref(), said);
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
                let won = seat.victory == Some(true);
                let said = format!(
                    "over at step {step}, floor {}, {}",
                    seat.floor,
                    match won {
                        true => "won",
                        false => "lost",
                    }
                );
                run.ending = match won {
                    true => Ending::Won,
                    false => Ending::Lost,
                };
                run.digests
                    .extend(seated.iter().map(|seat| position(&seat.snapshot)));
                let said = match &hunt {
                    None => said,
                    Some(hunt) => {
                        let parting = seated.iter().enumerate().find_map(|(at, seat)| {
                            let me = sts2_core::SeatIndex(u8::try_from(at).expect("a seat"));
                            let fields = hunt.compare(me, &seat.snapshot)?;
                            Some(format!("parted {step} seat {at} at the end in {fields}"))
                        });
                        match parting {
                            Some(parting) => {
                                run.ending = Ending::Parted;
                                parting
                            }
                            None => format!("agreed {step}, {said}"),
                        }
                    }
                };
                return finish(&coop, run, out.as_deref(), said);
            }
        }
    }
    for seat in coop.seats_mut() {
        run.digests.push(position(&seat.observe()?.snapshot));
    }
    if let Some(out) = &out {
        write(&coop, run, out)?;
    }
    Ok(format!("capped at {steps} steps"))
}

/// Writes `run` where `out` says, if it says, and says `said`.
fn finish<V: WireContentVersion>(
    coop: &Coop<V, GameSeat<V>>,
    run: Run,
    out: Option<&std::path::Path>,
    said: String,
) -> Result<String, Error<V>> {
    if let Some(out) = out {
        write(coop, run, out)?;
    }
    Ok(said)
}

/// A simulator seat beside each game seat, which `--hunt` plays.
///
/// Before each move it compares where the acting seat stands, and at the end
/// it compares every seat. The last line says `agreed`, or where the two
/// `parted`, or that a simulator `panicked`, `refused` or `failed`. The
/// game's run stops there, as a solo hunt's does.
struct Hunt<V: sts2_core::Rules> {
    sims: Vec<sts2_sim::SimSeat<V>>,
    /// The move the last comparison was before, which a parting comes after.
    after: String,
}

impl<V: sts2_core::Rules> Hunt<V> {
    /// A simulator seat for each of `start`'s players, or what the simulator
    /// panicked with as it began.
    fn begin(start: &Start<V>) -> Result<Self, String> {
        let sims = (0..start.players.len())
            .map(|at| {
                let me = sts2_core::SeatIndex(u8::try_from(at).expect("at most 256 seats"));
                caught(|| sts2_sim::SimSeat::begin(start, me))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            sims,
            after: "the start".into(),
        })
    }

    /// The sections where seat `me`'s simulator and `seen` differ, or nothing
    /// where they agree.
    fn compare(&self, me: sts2_core::SeatIndex, seen: &sts2_core::Snapshot<V>) -> Option<String> {
        let simulated = self.sims[me.at()].observe();
        (position(&simulated) != position(seen)).then(|| parted(&simulated, seen))
    }

    /// Seat `seat`'s `decision`, on every seat's simulator. A simulator that
    /// does not take it says how, `panicked`, `refused` or `failed`, with
    /// its seat and what it said.
    fn play(
        &mut self,
        seat: sts2_core::SeatIndex,
        decision: &sts2_core::Decision<V>,
    ) -> Result<(), (&'static str, sts2_core::SeatIndex, String)> {
        for sim in &mut self.sims {
            let me = sim.me();
            match caught(|| sim.play(seat, decision)).map_err(|why| ("panicked", me, why))? {
                Ok(Ok(_)) => {}
                Ok(Err(refusal)) => return Err(("refused", me, refusal.to_string())),
                Err(fault) => return Err(("failed", me, fault.to_string())),
            }
        }
        Ok(())
    }
}

/// `run`, or the first line of the message it panicked with.
fn caught<T>(run: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).map_err(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|text| text.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        message.lines().next().unwrap_or("").to_owned()
    })
}

/// Writes `run` to `out` as a file of one recorded run, stamped with the
/// build the host's game greeted with.
fn write<V: WireContentVersion>(
    coop: &Coop<V, GameSeat<V>>,
    run: Run,
    out: &std::path::Path,
) -> Result<(), Error<V>> {
    let greeting = coop.seats()[0].game().greeting();
    let recorded = Recorded {
        stamp: Stamp {
            assembly: greeting.assembly_hash,
            content: i64::from(greeting.content_hash),
        },
        runs: vec![run],
    };
    std::fs::write(out, recorded.encode()).map_err(Error::Write)
}
