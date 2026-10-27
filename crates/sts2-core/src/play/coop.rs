//! Seating a co-op run, and deciding who acts next.
//!
//! The seats do not take turns in the game. Each player acts when it likes,
//! and some moves wait on the others. [`Coop`] makes that one sequence: it
//! looks at every seat, and hands the next seat that has a move to the caller
//! to pick one.

use std::marker::PhantomData;

use super::seat::{Seat, SeatAnswered, SeatIndex, Seated};
use crate::{ContentVersion, Decision, Start};

/// How many times [`Coop::step`] looks again at a seat that has not caught
/// up with the others before it calls the run stalled. Each look waits for
/// the seat's game to stand still, so this is a few seconds at most.
const CATCH_UP: usize = 1000;

/// The seats of one co-op run.
pub struct Coop<V: ContentVersion, S: Seat<V>> {
    seats: Vec<S>,
    /// The seat looked at first on the next step, so every seat gets a turn.
    next: usize,
    version: PhantomData<V>,
}

/// What one step of a co-op run did.
#[derive(Debug, Clone, PartialEq)]
pub enum Turn<V: ContentVersion> {
    /// A seat played a move.
    Played {
        seat: SeatIndex,
        decision: Decision<V>,
        /// Boxed, because a seat's answer is large and the other turns are not.
        answered: Box<SeatAnswered<V>>,
    },
    /// No seat has a move, and the run goes on, or a seat never took in what
    /// another had. Each seat, as it stands.
    Stalled(Vec<Seated<V>>),
    /// The run is over on every seat. Each seat, as it stands.
    Over(Vec<Seated<V>>),
}

impl<V: ContentVersion, S: Seat<V>> Coop<V, S> {
    /// Seats a player at each host, in seat order, and returns once the run
    /// has started on all of them.
    ///
    /// Every seat begins at once, on a thread of its own, because the host's
    /// run starts only when the others have joined.
    ///
    /// # Panics
    ///
    /// When `hosts` and the run's players differ in number, or there are
    /// more than 256 seats.
    pub fn begin(hosts: Vec<S::Host>, start: &Start<V>) -> Result<Self, S::Error>
    where
        S: Send,
        S::Host: Send,
        S::Error: Send,
    {
        assert_eq!(hosts.len(), start.players.len(), "one host for each player");
        let begun: Vec<Result<S, S::Error>> = std::thread::scope(|scope| {
            let threads: Vec<_> = hosts
                .into_iter()
                .enumerate()
                .map(|(at, host)| {
                    let me = SeatIndex(u8::try_from(at).expect("at most 256 seats"));
                    scope.spawn(move || S::begin(host, start, me))
                })
                .collect();
            threads
                .into_iter()
                .map(|thread| thread.join().expect("a seat's begin does not panic"))
                .collect()
        });
        let seats = begun.into_iter().collect::<Result<Vec<S>, S::Error>>()?;
        Ok(Self {
            seats,
            next: 0,
            version: PhantomData,
        })
    }

    /// Every seat, in seat order.
    pub fn seats(&self) -> &[S] {
        &self.seats
    }

    /// Every seat, in seat order, to ask what only its own type knows.
    pub fn seats_mut(&mut self) -> &mut [S] {
        &mut self.seats
    }

    /// Looks at every seat, and plays one move on the first that has one.
    ///
    /// The seats are looked at in turn, from the one after the seat that last
    /// played. `choose` is given that seat, where it stands, and its moves. It
    /// returns the index of the move to play.
    ///
    /// # Panics
    ///
    /// When `choose` returns an index outside the moves it was given.
    pub fn step(
        &mut self,
        mut choose: impl FnMut(SeatIndex, &Seated<V>, &[Decision<V>]) -> usize,
    ) -> Result<Turn<V>, S::Error> {
        let mut seated = self
            .seats
            .iter_mut()
            .map(Seat::observe)
            .collect::<Result<Vec<_>, _>>()?;
        // Another seat's move reaches a seat over the network, so a seat can
        // be looked at before it has taken in what another has. Each one
        // behind is looked at again until every seat has taken in as much as
        // the furthest.
        let mut looks = 0;
        loop {
            let furthest = seated.iter().map(|seat| seat.peers.synced).max();
            let mut behind = false;
            for (seat, at) in self.seats.iter_mut().zip(&mut seated) {
                if Some(at.peers.synced) < furthest {
                    *at = seat.observe()?;
                    behind = true;
                }
            }
            if !behind {
                break;
            }
            looks += 1;
            if looks == CATCH_UP {
                return Ok(Turn::Stalled(seated));
            }
        }
        let count = self.seats.len();
        let acting = (0..count)
            .map(|offset| (self.next + offset) % count)
            .map(|at| (at, SeatIndex(u8::try_from(at).expect("at most 256 seats"))))
            .find(|&(at, me)| {
                !seated[at].snapshot.observation.run_over && !seated[at].moves(me).is_empty()
            });
        let Some((at, me)) = acting else {
            return Ok(
                match seated.iter().all(|seat| seat.snapshot.observation.run_over) {
                    true => Turn::Over(seated),
                    false => Turn::Stalled(seated),
                },
            );
        };
        let moves = seated[at].moves(me);
        let decision = moves[choose(me, &seated[at], moves)];
        let answered = self.seats[at].play(&decision)?;
        self.next = (at + 1) % count;
        Ok(Turn::Played {
            seat: me,
            decision,
            answered: Box::new(answered),
        })
    }
}
