//! One runner for every way this repository plays a run.
//!
//! A run is played at a [`RunTable`], the simulator's or the game's. A
//! [`Player`] picks each seat's moves. Layers wrap the table, each doing one
//! job, such as [`Carried`]. The driver plays until the run ends, and says
//! how in one [`Verdict`]. `docs/RUNNERS.md` sets out the whole of it.

pub mod carried;
pub mod ids;
pub mod random;

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use num_traits::ToPrimitive as _;
use sts2_core::{
    ContentVersion, Decision, End, Gift, Observed as _, RunTable, SeatIndex, Settled, Setup,
    Snapshot, Start, Verdict,
};

/// What picks one seat's moves.
pub trait Player<V: ContentVersion> {
    /// Which of `moves` seat `seat` makes. A player that reads more than the
    /// moves asks `table`, which builds only what it is asked for.
    fn choose<T: RunTable<V>>(
        &mut self,
        seat: SeatIndex,
        table: &mut T,
        moves: &[Decision<V>],
    ) -> Result<usize, T::Error>;
}

/// Moves at random, as [`random::Pick`] picks them: the census's and the
/// recorder's player.
pub struct Random(random::Pick);

impl Random {
    /// The player a run's seed starts.
    pub fn for_seed(seed: &str) -> Self {
        Self(random::Pick::for_seed(seed))
    }
}

impl<V: ContentVersion> Player<V> for Random {
    /// The question is built only where a discard makes room, which a
    /// full belt on a reward screen is.
    fn choose<T: RunTable<V>>(
        &mut self,
        seat: SeatIndex,
        table: &mut T,
        moves: &[Decision<V>],
    ) -> Result<usize, T::Error> {
        let (held, slots) = {
            let seen = table.seen(seat)?;
            (seen.potions().len(), seen.potion_slots())
        };
        let full = slots.to_usize().is_some_and(|slots| held >= slots);
        let room = full
            && moves
                .iter()
                .any(|decision| matches!(decision, Decision::DiscardPotion { .. }))
            && {
                let asking = table.snapshot(seat)?.asking;
                random::making_room(asking.as_ref(), held, slots)
            };
        Ok(self.0.choose(moves, room))
    }
}

/// Why a run stopped short of a verdict of its own: a table's fault, or a
/// layer's refusal. It ends the run as its `end` says.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{detail}")]
pub struct Fault {
    pub end: End,
    pub detail: String,
}

impl From<sts2_sim::Error> for Fault {
    fn from(error: sts2_sim::Error) -> Self {
        Self {
            end: End::Failed,
            detail: error.to_string(),
        }
    }
}

/// A table whose every player carries [`carried::carried`] from before the
/// first move. That is 25 Gorgets and a Mercury Hourglass, so play at random
/// reaches content instead of dying early.
///
/// A solo run grants its own player. A run of several players has each
/// seat's machine make every grant, for every player, in seat order, as each
/// game does.
pub struct Carried<T>(pub T);

impl<V, T> RunTable<V> for Carried<T>
where
    V: sts2_content::Build,
    T: RunTable<V>,
    Fault: From<T::Error>,
{
    type Error = Fault;
    type Host = T::Host;
    type Seen<'a>
        = T::Seen<'a>
    where
        Self: 'a;

    fn begin(host: Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        let mut table = T::begin(host, start)?;
        let seats = table.seats();
        let grants: Vec<Setup<V>> = match seats {
            1 => carried::carried::<V>()
                .into_iter()
                .flat_map(|(relic, count)| {
                    std::iter::repeat_n(Setup::Grant(Gift::Relic(relic)), count)
                })
                .collect(),
            seats => carried::carried_by_every_seat::<V>(seats),
        };
        for machine in SeatIndex::all(seats) {
            for grant in &grants {
                table.construct(machine, grant)?.map_err(|refusal| Fault {
                    end: End::Refused,
                    detail: format!("seat {}'s machine refused {grant:?}: {refusal}", machine.0),
                })?;
            }
        }
        Ok(Self(table))
    }

    fn seats(&self) -> usize {
        self.0.seats()
    }

    fn seen(&mut self, seat: SeatIndex) -> Result<Self::Seen<'_>, Self::Error> {
        Ok(self.0.seen(seat)?)
    }

    fn moves(&mut self, seat: SeatIndex, moves: &mut Vec<Decision<V>>) -> Result<(), Self::Error> {
        Ok(self.0.moves(seat, moves)?)
    }

    fn play(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Self::Error> {
        Ok(self.0.play(seat, decision)?)
    }

    fn snapshot(&mut self, seat: SeatIndex) -> Result<Snapshot<V>, Self::Error> {
        Ok(self.0.snapshot(seat)?)
    }

    fn construct(&mut self, seat: SeatIndex, setup: &Setup<V>) -> Result<Settled<V>, Self::Error> {
        Ok(self.0.construct(seat, setup)?)
    }

    fn logged(&mut self) -> Vec<(SeatIndex, String)> {
        self.0.logged()
    }
}

/// How far the run in this process has got, for the verdict the panic hook
/// prints. That is the moves played, and the furthest act and floor any
/// seat reached.
static STEP: AtomicUsize = AtomicUsize::new(0);
static ACT: AtomicU32 = AtomicU32::new(0);
static FLOOR: AtomicU32 = AtomicU32::new(0);

/// A verdict on the run in this process, as far as it got.
fn verdict(end: End, seat: Option<SeatIndex>, place: Option<String>, detail: String) -> Verdict {
    Verdict {
        end,
        step: STEP.load(Ordering::Relaxed),
        act: ACT.load(Ordering::Relaxed),
        floor: FLOOR.load(Ordering::Relaxed),
        seat,
        place,
        detail,
    }
}

/// Prints a [`End::Panicked`] verdict on stdout when the process panics,
/// before the panic's own report on stderr. An unported path panics, and
/// the process ends there.
pub fn print_verdict_on_panic() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|message| (*message).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let place = info.location().map(ToString::to_string);
        println!("{}", verdict(End::Panicked, None, place, message));
        report(info);
    }));
}

/// Plays the run at `table`, with `player` choosing for every seat, until it
/// ends or `cap` moves are played. How it ended.
///
/// The seats are looked at in turn, from the one after the seat that last
/// moved. The first that is still in the run and has a move plays one. A run
/// where no seat has a move is over once every seat says so, and stalled
/// otherwise.
pub fn drive<V, T, P>(table: &mut T, player: &mut P, cap: usize) -> Verdict
where
    V: ContentVersion,
    T: RunTable<V>,
    P: Player<V>,
    Fault: From<T::Error>,
{
    match drive_or_fault(table, player, cap) {
        Ok(verdict) => verdict,
        Err(fault) => verdict(fault.end, None, None, fault.detail),
    }
}

fn drive_or_fault<V, T, P>(table: &mut T, player: &mut P, cap: usize) -> Result<Verdict, Fault>
where
    V: ContentVersion,
    T: RunTable<V>,
    P: Player<V>,
    Fault: From<T::Error>,
{
    let seats = table.seats();
    let mut moves = Vec::new();
    let mut next = 0;
    for step in 0..cap {
        STEP.store(step, Ordering::Relaxed);
        let mut over = 0;
        let mut acting = None;
        for offset in 0..seats {
            let seat = SeatIndex(u8::try_from((next + offset) % seats).expect("at most 256 seats"));
            let run_over = {
                let seen = table.seen(seat)?;
                ACT.fetch_max(seen.act().to_u32().unwrap_or(0), Ordering::Relaxed);
                FLOOR.fetch_max(seen.floor().to_u32().unwrap_or(0), Ordering::Relaxed);
                seen.run_over()
            };
            if run_over {
                over += 1;
                continue;
            }
            if acting.is_none() {
                table.moves(seat, &mut moves)?;
                if !moves.is_empty() {
                    acting = Some(seat);
                }
            }
        }
        let Some(seat) = acting else {
            if over < seats {
                return Ok(verdict(End::Stalled, None, None, String::new()));
            }
            let won = table.seen(SeatIndex(0))?.victory() == Some(true);
            let end = if won { End::Won } else { End::Lost };
            return Ok(verdict(end, None, None, String::new()));
        };
        // `moves` holds the acting seat's, since the look stops asking once it
        // finds one.
        let at = player.choose(seat, table, &moves)?;
        let decision = moves[at];
        if let Err(refusal) = table.play(seat, &decision)? {
            let detail = format!("{decision:?}: {refusal}");
            return Ok(verdict(End::Refused, Some(seat), None, detail));
        }
        next = (seat.at() + 1) % seats;
    }
    STEP.store(cap, Ordering::Relaxed);
    Ok(verdict(End::Capped, None, None, String::new()))
}
