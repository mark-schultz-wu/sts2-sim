//! One runner for every way this repository plays a run.
//!
//! A run is played at a [`RunTable`], the simulator's or the game's. A
//! [`Player`] picks each seat's moves. Layers wrap the table, each doing one
//! job, such as [`Carried`]. The driver plays until the run ends, and says
//! how in one [`Verdict`]. `docs/RUNNERS.md` sets out the whole of it.

pub mod carried;
pub mod ids;
pub mod layers;
pub mod random;
pub mod recorded;

pub use layers::{Carried, Hunted, LogGuard, MovesLog, Recorded};

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use num_traits::ToPrimitive as _;
use sts2_core::{
    ContentVersion, Decision, End, Observed as _, RunTable, SeatIndex, Settled, Start, Verdict,
};

/// What picks one seat's moves.
pub trait Player<V: ContentVersion> {
    /// Seat `seat`'s move, or nothing for a player who stops the run.
    ///
    /// `moves` is what the table lists. A player can choose a move it does
    /// not list, as a person at a terminal can. The table says why not, and
    /// [`Player::answered`] hears it. A player that reads more than the moves
    /// asks `table`, which builds only what it is asked for.
    fn choose<T: RunTable<V>>(
        &mut self,
        seat: SeatIndex,
        table: &mut T,
        moves: &[Decision<V>],
    ) -> Result<Option<Decision<V>>, T::Error>;

    /// What the table answered the move this player chose: what it decided,
    /// or why it refused it.
    fn answered(&mut self, _seat: SeatIndex, _decision: &Decision<V>, _settled: &Settled<V>) {}

    /// How the run ended, at the table where it ended.
    fn ended<T: RunTable<V>>(&mut self, _table: &mut T, _verdict: &Verdict) {}
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
    ) -> Result<Option<Decision<V>>, T::Error> {
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
        Ok(Some(moves[self.0.choose(moves, room)]))
    }
}

/// Every legal move alike, backing out of a selection and throwing a potion
/// away among them. Random play that leaves nothing out reaches states
/// nobody builds by hand, where a move the table listed and then refuses
/// shows.
pub struct Uniform(random::Pick);

impl Uniform {
    /// The player a run's seed starts.
    pub fn for_seed(seed: &str) -> Self {
        Self(random::Pick::for_seed(seed))
    }
}

impl<V: ContentVersion> Player<V> for Uniform {
    fn choose<T: RunTable<V>>(
        &mut self,
        _: SeatIndex,
        _: &mut T,
        moves: &[Decision<V>],
    ) -> Result<Option<Decision<V>>, T::Error> {
        Ok(Some(moves[self.0.any(moves.len())]))
    }
}

/// Why a run stopped short of a verdict of its own: a table's fault, or a
/// layer's refusal. It ends the run as its `end` says.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{detail}")]
pub struct Fault {
    pub end: End,
    /// The seat it is about, where it is about one.
    pub seat: Option<SeatIndex>,
    pub detail: String,
}

impl From<sts2_sim::Error> for Fault {
    fn from(error: sts2_sim::Error) -> Self {
        Self {
            end: End::Failed,
            seat: None,
            detail: error.to_string(),
        }
    }
}

/// A game that went away, or stopped answering, ends the run as
/// [`End::GameGone`], which a caller plays again. Any other fault of the
/// game's is [`End::Failed`].
impl From<sts2_headless::Error> for Fault {
    fn from(error: sts2_headless::Error) -> Self {
        let end = match error {
            sts2_headless::Error::GameGone | sts2_headless::Error::Timeout(_) => End::GameGone,
            _ => End::Failed,
        };
        Self {
            end,
            seat: None,
            detail: error.to_string(),
        }
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

/// Where the last panic was raised, and what it said, for the verdict.
static PANICKED: std::sync::Mutex<Option<(Option<String>, String)>> = std::sync::Mutex::new(None);

/// Keeps where each panic was raised and what it said, for [`run`]'s
/// verdict, before the panic's own report on stderr.
pub fn keep_panics() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|message| (*message).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let place = info.location().map(ToString::to_string);
        if let Ok(mut panicked) = PANICKED.try_lock() {
            *panicked = Some((place, message));
        }
        report(info);
    }));
}

/// The verdict on a panic [`keep_panics`] kept.
fn panicked() -> Verdict {
    let (place, message) = taken_panic();
    verdict(End::Panicked, None, place, message)
}

/// Where the last panic [`keep_panics`] kept was raised, and what it said.
fn taken_panic() -> (Option<String>, String) {
    PANICKED
        .lock()
        .ok()
        .and_then(|mut panicked| panicked.take())
        .unwrap_or_default()
}

/// `act`, or where it panicked and what it said. A caller that catches one
/// reports it and plays on no further.
pub fn catching<R>(act: impl FnOnce() -> R) -> Result<R, (Option<String>, String)> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(act)).map_err(|_| taken_panic())
}

/// Starts the run `start` names at a table of `T`, plays it with `player`
/// for at most `cap` moves, and finishes it: its verdict.
///
/// A panic stops the run and is its verdict. It is caught once, here, so the
/// table's layers still finish: a recorded run is written with where each
/// seat stood. Nothing plays on after one.
pub fn run<V, T, P>(host: T::Host, start: &Start<V>, player: &mut P, cap: usize) -> Verdict
where
    V: ContentVersion,
    T: RunTable<V>,
    P: Player<V>,
    Fault: From<T::Error>,
{
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let begun = catch_unwind(AssertUnwindSafe(|| T::begin(host, start)));
    let mut table = match begun {
        Ok(Ok(table)) => table,
        Ok(Err(fault)) => {
            let fault = Fault::from(fault);
            let detail = format!("as the run began: {}", fault.detail);
            return verdict(fault.end, fault.seat, None, detail);
        }
        Err(_) => return panicked(),
    };
    play_at(&mut table, player, cap)
}

/// [`run`], at a table whose run is already under way, as a daemon holds
/// one.
pub fn play_at<V, T, P>(table: &mut T, player: &mut P, cap: usize) -> Verdict
where
    V: ContentVersion,
    T: RunTable<V>,
    P: Player<V>,
    Fault: From<T::Error>,
{
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let driven = catch_unwind(AssertUnwindSafe(|| drive(table, &mut *player, cap)));
    let verdict = driven.unwrap_or_else(|_| panicked());
    let _ = catch_unwind(AssertUnwindSafe(|| player.ended(table, &verdict)));
    catch_unwind(AssertUnwindSafe(|| table.finish(verdict.clone()))).unwrap_or_else(|_| panicked())
}

/// Plays the run at `table`, with `player` choosing for every seat, until it
/// ends or `cap` moves are played. How it ended.
///
/// The seats are looked at in turn, from the one after the seat that last
/// moved. The first that is still in the run and has a move plays one. A run
/// where no seat has a move is over once every seat says so, and stalled
/// otherwise.
fn drive<V, T, P>(table: &mut T, player: &mut P, cap: usize) -> Verdict
where
    V: ContentVersion,
    T: RunTable<V>,
    P: Player<V>,
    Fault: From<T::Error>,
{
    match drive_or_fault(table, player, cap) {
        Ok(verdict) => verdict,
        Err(fault) => verdict(fault.end, fault.seat, None, fault.detail),
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
    let mut step = 0;
    while step < cap {
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
        let Some(decision) = player.choose(seat, table, &moves)? else {
            return Ok(verdict(End::Quit, Some(seat), None, String::new()));
        };
        let settled = table.play(seat, &decision)?;
        player.answered(seat, &decision, &settled);
        if let Err(refusal) = settled {
            // A move the table listed and then refused is a fault in one or
            // the other. One it never listed was the player's to try, and
            // the player is asked again.
            if moves.contains(&decision) {
                let detail = format!("{decision:?}: {refusal}");
                return Ok(verdict(End::Refused, Some(seat), None, detail));
            }
            continue;
        }
        next = (seat.at() + 1) % seats;
        step += 1;
    }
    STEP.store(cap, Ordering::Relaxed);
    Ok(verdict(End::Capped, None, None, String::new()))
}
