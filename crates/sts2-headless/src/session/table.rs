//! A run at a table, on the real game: one game process for each seat.
//!
//! A solo run is [`OracleTable`], one game launched for one player. A run of
//! several players is [`LobbyTable`], one game for each seat over ENet, as
//! [`GameSeat`] launches them. `docs/RUNNERS.md` says how a runner drives
//! either.

use sts2_core::{
    Decided, Decision, Observation, RunTable, SeatIndex, Seated, Settled, Setup, Snapshot, Start,
};

use crate::session::error::Error;
use crate::{Config, GameSeat, JoinOrder, Oracle, SeatHost};

/// A solo run on one game.
#[derive(Debug)]
pub struct OracleTable<V: sts2_content_wire::WireContentVersion> {
    game: Oracle<V>,
    /// Where the run stands, as the game's last reply said.
    last: Snapshot<V>,
    /// What the game logged in every reply since [`RunTable::logged`].
    logged: Vec<String>,
}

impl<V: sts2_content_wire::WireContentVersion> OracleTable<V> {
    /// The table's game. See [`Oracle`].
    pub fn game(&self) -> &Oracle<V> {
        &self.game
    }

    /// A reply's answer, keeping where it left the run and what the game
    /// logged.
    fn settled(&mut self, answered: sts2_core::Answered<V>) -> Settled<V> {
        self.logged.extend_from_slice(self.game.logged());
        let (decided, snapshot) = answered?.split();
        self.last = snapshot;
        Ok(decided)
    }
}

impl<V: sts2_content_wire::WireContentVersion> RunTable<V> for OracleTable<V> {
    type Error = Error;
    type Host = Config;
    type Seen<'a> = &'a Observation<V>;

    /// Launches the game and starts the run.
    ///
    /// # Panics
    ///
    /// On a run of several players, which is a [`LobbyTable`]'s.
    fn begin(host: Config, start: &Start<V>) -> Result<Self, Error> {
        start.alone();
        let mut game = Oracle::begin(host, start)?;
        let last = game.observe()?;
        Ok(Self {
            game,
            last,
            logged: Vec::new(),
        })
    }

    fn seats(&self) -> usize {
        1
    }

    fn seen(&mut self, _: SeatIndex) -> Result<Self::Seen<'_>, Error> {
        Ok(&self.last.observation)
    }

    fn moves(&mut self, _: SeatIndex, moves: &mut Vec<Decision<V>>) -> Result<(), Error> {
        moves.clear();
        moves.extend_from_slice(&self.last.observation.moves);
        Ok(())
    }

    fn play(&mut self, _: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Error> {
        let answered = self.game.play(decision)?;
        Ok(self.settled(answered))
    }

    fn snapshot(&mut self, _: SeatIndex) -> Result<Snapshot<V>, Error> {
        Ok(self.last.clone())
    }

    fn construct(&mut self, _: SeatIndex, setup: &Setup<V>) -> Result<Settled<V>, Error> {
        let answered = self.game.setup(setup)?;
        Ok(self.settled(answered))
    }

    fn logged(&mut self) -> Vec<(SeatIndex, String)> {
        std::mem::take(&mut self.logged)
            .into_iter()
            .map(|error| (SeatIndex(0), error))
            .collect()
    }
}

/// The file a client's game writes in its data home once it has joined.
const JOINED: &str = "coop-joined";

/// How many times a lobby looks again at seats that have no move and are not
/// all over. Each game runs on its own, so a move one seat sent can still be
/// on its way to the others.
const STALLS: usize = 20;

/// How many times a lobby looks at every seat again before it calls the
/// seats stalled. Each look waits for the seat's game to stand still, so
/// this is a few seconds at most.
const CATCH_UP: usize = 1000;

/// Every seat as it stands, once each caught up with the others, or as they
/// stood when one never did.
type CaughtUp<V> = Result<Vec<Seated<V>>, Vec<Seated<V>>>;

/// Whether every game stands still with nothing on its way between them.
///
/// A game alone cannot tell a packet is on its way to it. So each counts
/// the reliable packets it sent to each other game, and those it took in
/// from each. Where each end of every link took in what the other sent,
/// no packet is on its way.
fn quiet<V: sts2_core::ContentVersion>(seated: &[Seated<V>]) -> bool {
    let still = seated.iter().all(|seat| seat.peers.still);
    let balanced = (seated.iter().enumerate()).all(|(from, seat)| {
        (seat.peers.links.iter().enumerate())
            .filter(|&(to, _)| to != from)
            .all(|(to, link)| link.sent == seated[to].peers.links[from].received)
    });
    still && balanced
}

/// A run of several players, one game for each seat.
pub struct LobbyTable<V: sts2_content_wire::WireContentVersion> {
    seats: Vec<GameSeat<V>>,
    /// Where every seat stood when last looked at, once each had caught up.
    /// Nothing once a move or a setup has changed the run.
    seated: Option<Vec<Seated<V>>>,
}

impl<V: sts2_content_wire::WireContentVersion> LobbyTable<V>
where
    Seated<V>: Send,
{
    /// Each seat's game. See [`GameSeat`].
    pub fn seats(&self) -> &[GameSeat<V>] {
        &self.seats
    }

    /// Where the seats at `which` stand, each asked of its own game. The games
    /// are processes of their own, so they are asked at once.
    fn looked_at(&mut self, which: &[usize]) -> Result<Vec<(usize, Seated<V>)>, Error> {
        if let [one] = which {
            return Ok(vec![(*one, self.seats[*one].observe()?)]);
        }
        let looked: Vec<(usize, Result<Seated<V>, Error>)> = std::thread::scope(|scope| {
            let asked: Vec<_> = (self.seats.iter_mut().enumerate())
                .filter(|(at, _)| which.contains(at))
                .map(|(at, seat)| (at, scope.spawn(move || seat.observe())))
                .collect();
            asked
                .into_iter()
                .map(|(at, asked)| (at, asked.join().expect("a seat's look does not panic")))
                .collect()
        });
        looked
            .into_iter()
            .map(|(at, seated)| Ok((at, seated?)))
            .collect()
    }

    /// Where every seat stands, once nothing is on its way between them.
    ///
    /// A still game moves only when a packet or a command reaches it. So
    /// once every game is still and [`quiet`] finds every link balanced, the
    /// run stands still on every machine. Until then every seat is looked
    /// at again. Seats that never settle are the error, as they stood.
    fn caught_up(&mut self) -> Result<CaughtUp<V>, Error> {
        let every: Vec<usize> = (0..self.seats.len()).collect();
        let mut seated: Vec<Seated<V>> = Vec::new();
        for _ in 0..CATCH_UP {
            seated = (self.looked_at(&every)?)
                .into_iter()
                .map(|(_, seated)| seated)
                .collect();
            if quiet(&seated) {
                return Ok(Ok(seated));
            }
        }
        Ok(Err(seated))
    }

    /// Where every seat stands, looked at once each has caught up with the
    /// others. They are looked at again while no seat has a move and the run
    /// is not over on all of them. Seats that stall stand as last seen.
    fn settled(&mut self) -> Result<&[Seated<V>], Error> {
        if self.seated.is_none() {
            let mut seated = Vec::new();
            for _ in 0..STALLS {
                seated = match self.caught_up()? {
                    Ok(seated) => seated,
                    Err(stalled) => {
                        seated = stalled;
                        continue;
                    }
                };
                let moving = seated
                    .iter()
                    .zip(SeatIndex::all(seated.len()))
                    .any(|(at, me)| !at.snapshot.observation.run_over && !at.moves(me).is_empty());
                let over = seated.iter().all(|at| at.snapshot.observation.run_over);
                if moving || over {
                    break;
                }
            }
            self.seated = Some(seated);
        }
        Ok(self
            .seated
            .as_deref()
            .expect("the seats were just looked at"))
    }
}

/// The seats are looked at on threads of their own, so where one stands has
/// to cross from one to another, as every build's does.
impl<V: sts2_content_wire::WireContentVersion> RunTable<V> for LobbyTable<V>
where
    Seated<V>: Send,
    sts2_core::Answered<V>: Send,
    Setup<V>: Sync,
{
    type Error = Error;
    type Host = Vec<SeatHost>;
    type Seen<'a> = &'a Observation<V>;

    /// Launches a game for each seat, which host or join, and returns once
    /// the run has started on all of them.
    ///
    /// Every seat begins at once, on a thread of its own, because the host's
    /// run starts only when the others have joined.
    ///
    /// # Panics
    ///
    /// When the hosts and the run's players differ in number.
    fn begin(host: Vec<SeatHost>, start: &Start<V>) -> Result<Self, Error> {
        assert_eq!(host.len(), start.players.len(), "one host for each player");
        // The lobby seats each client in the order it joins, and the run's
        // players follow it. So each client joins once the one before it
        // has, which it marks in its own data home.
        let joined: Vec<Option<std::path::PathBuf>> = (host.iter())
            .map(|host| host.config.data_home.as_ref().map(|home| home.join(JOINED)))
            .collect();
        for marker in joined.iter().flatten() {
            match std::fs::remove_file(marker) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(Error::Io("removing a join marker", e));
                }
                _ => {}
            }
        }
        let begun: Vec<Result<GameSeat<V>, Error>> = std::thread::scope(|scope| {
            let threads: Vec<_> = (host.into_iter().zip(SeatIndex::all(start.players.len())))
                .map(|(host, me)| {
                    let order = JoinOrder {
                        after: (me.at() > 1).then(|| joined[me.at() - 1].clone()).flatten(),
                        joined: (me.at() > 0).then(|| joined[me.at()].clone()).flatten(),
                    };
                    scope.spawn(move || GameSeat::begin(host, start, me, order))
                })
                .collect();
            threads
                .into_iter()
                .map(|thread| thread.join().expect("a seat's begin does not panic"))
                .collect()
        });
        Ok(Self {
            seats: begun.into_iter().collect::<Result<_, _>>()?,
            seated: None,
        })
    }

    fn seats(&self) -> usize {
        self.seats.len()
    }

    fn seen(&mut self, seat: SeatIndex) -> Result<Self::Seen<'_>, Error> {
        Ok(&self.settled()?[seat.at()].snapshot.observation)
    }

    fn moves(&mut self, seat: SeatIndex, moves: &mut Vec<Decision<V>>) -> Result<(), Error> {
        moves.clear();
        moves.extend_from_slice(self.settled()?[seat.at()].moves(seat));
        Ok(())
    }

    /// A seat's game says nothing of a move abandoned under a fight's end,
    /// so every move it takes is resolved here.
    fn play(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Error> {
        self.seated = None;
        let answered = self.seats[seat.at()].play(decision)?;
        Ok(answered.map(|_| Decided::Resolved))
    }

    fn snapshot(&mut self, seat: SeatIndex) -> Result<Snapshot<V>, Error> {
        Ok(self.settled()?[seat.at()].snapshot.clone())
    }

    fn construct(&mut self, seat: SeatIndex, setup: &Setup<V>) -> Result<Settled<V>, Error> {
        self.seated = None;
        let answered = self.seats[seat.at()].setup(setup)?;
        Ok(answered.map(|applied| applied.split().0))
    }

    /// Every game at once, on a thread each: a game entering a fight waits
    /// for the others to enter it too.
    fn construct_everywhere(&mut self, setup: &Setup<V>) -> Result<Vec<Settled<V>>, Error> {
        self.seated = None;
        let answered: Vec<Result<sts2_core::Answered<V>, Error>> = std::thread::scope(|scope| {
            let threads: Vec<_> = (self.seats.iter_mut())
                .map(|seat| scope.spawn(move || seat.setup(setup)))
                .collect();
            threads
                .into_iter()
                .map(|thread| thread.join().expect("a seat's setup does not panic"))
                .collect()
        });
        answered
            .into_iter()
            .map(|answered| Ok(answered?.map(|applied| applied.split().0)))
            .collect()
    }

    fn logged(&mut self) -> Vec<(SeatIndex, String)> {
        let count = self.seats.len();
        self.seats
            .iter_mut()
            .zip(SeatIndex::all(count))
            .flat_map(|(seat, me)| seat.take_logged().into_iter().map(move |error| (me, error)))
            .collect()
    }
}
