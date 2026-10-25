//! The co-op driver, over seats that each play a run of their own.
//!
//! A [`Solo`] seat is the simulator playing one player's run alone, with no
//! other seat in it. That is no co-op run. It is enough to hold the driver to
//! its order: who acts, who waits, and when the run is over.

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{
    Coop, Decision, Lobby, Peer, Peers, Play, Seat, SeatAnswered, SeatIndex, Seated, Seed, Start,
    Turn,
};
use sts2_sim::Sim;

type V = V0_107_1;
type Character = <V as sts2_core::ContentVersion>::Character;
type AscensionLevel = <V as sts2_core::ContentVersion>::AscensionLevel;

/// One player's run on the simulator, as a seat with no others to wait on.
struct Solo {
    sim: Sim<V>,
    seats: usize,
}

impl Solo {
    fn peers(&self) -> Peers {
        Peers {
            seats: vec![Peer::default(); self.seats],
            awaiting: Vec::new(),
            in_flight: false,
        }
    }
}

impl Seat<V> for Solo {
    type Error = sts2_sim::Error;
    type Host = ();

    fn begin(_: (), lobby: &Lobby<V>, me: SeatIndex) -> Result<Self, Self::Error> {
        let start = Start {
            seed: lobby.seed.clone(),
            character: lobby.characters[me.at()],
            ascension: AscensionLevel::None,
            modifiers: Vec::new(),
            wins: 0,
        };
        Ok(Self {
            sim: Sim::begin((), &start)?,
            seats: lobby.characters.len(),
        })
    }

    fn play(&mut self, decision: &Decision<V>) -> Result<SeatAnswered<V>, Self::Error> {
        let peers = self.peers();
        Ok(self.sim.play(decision)?.map(|applied| Seated {
            snapshot: applied.snapshot(),
            peers,
        }))
    }

    fn observe(&mut self) -> Result<Seated<V>, Self::Error> {
        Ok(Seated {
            snapshot: self.sim.observe()?,
            peers: self.peers(),
        })
    }
}

fn lobby() -> Lobby<V> {
    Lobby {
        seed: Seed::new("C0000001").expect("a seed"),
        characters: vec![Character::Ironclad, Character::Silent],
    }
}

fn coop() -> Coop<V, Solo> {
    Coop::begin(vec![(), ()], &lobby()).expect("the seats begin")
}

#[test]
fn the_seats_take_turns() {
    let mut coop = coop();
    let mut acted = Vec::new();
    for _ in 0..6 {
        match coop.step(|_, _, _| 0).expect("a step") {
            Turn::Played { seat, .. } => acted.push(seat.0),
            other => panic!("a seat with moves did not act: {other:?}"),
        }
    }
    assert_eq!(acted, [0, 1, 0, 1, 0, 1]);
}

#[test]
fn a_seat_that_ended_its_turn_or_voted_waits() {
    let mut coop = coop();
    let mut seated = match coop.step(|_, _, _| 0).expect("a step") {
        Turn::Played { answered, .. } => answered.expect("the move is accepted"),
        other => panic!("a seat with moves did not act: {other:?}"),
    };
    let me = SeatIndex(0);
    assert!(!seated.moves(me).is_empty());

    seated.peers.seats[0].ended_turn = true;
    assert!(seated.moves(me).is_empty());
    assert!(!seated.moves(SeatIndex(1)).is_empty());

    seated.peers.seats[0] = Peer {
        ended_turn: false,
        vote: Some(sts2_core::Coord::new(0, 1)),
    };
    assert!(seated.moves(me).is_empty());

    seated.peers.seats[0] = Peer::default();
    seated.peers.in_flight = true;
    assert!(seated.moves(me).is_empty());
}

#[test]
fn the_run_is_over_once_it_is_over_on_every_seat() {
    let mut coop = coop();
    let mut over = None;
    for step in 0..20_000_usize {
        match coop
            .step(|_, _, moves| step.wrapping_mul(7919) % moves.len())
            .expect("a step")
        {
            Turn::Played { answered, .. } => {
                answered.expect("a listed move is accepted");
            }
            Turn::Stalled(_) => panic!("a seat of its own never waits"),
            Turn::Over(seated) => {
                over = Some(seated);
                break;
            }
        }
    }
    let seated = over.expect("both runs end");
    assert!(seated.iter().all(|seat| seat.snapshot.observation.run_over));
}
