//! The driver, over the simulator's table of two seats.
//!
//! Who acts, who waits, and when the run is over: the order `docs/RUNNERS.md`
//! gives the driver, held on a run of two players.

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{
    Decision, End, Peer, Peers, Player, RunTable, SeatIndex, Seated, Seed, Start, Verdict,
};
use sts2_sim::{Sim, SimTable};

type V = V0_107_1;
type Character = <V as sts2_core::ContentVersion>::Character;
type AscensionLevel = <V as sts2_core::ContentVersion>::AscensionLevel;

fn lobby() -> Start<V> {
    let player = |character| Player { character, wins: 0 };
    Start {
        seed: Seed::new("C0000001").expect("a seed"),
        players: vec![player(Character::Ironclad), player(Character::Silent)],
        ascension: AscensionLevel::None,
        modifiers: Vec::new(),
    }
}

/// A player that notes the seat of each move it is asked for, and picks by
/// a fixed rule from the moves listed.
struct Noting {
    seats: Vec<u8>,
    pick: fn(usize, usize) -> usize,
}

impl sts2_run::Player<V> for Noting {
    fn choose<T: RunTable<V>>(
        &mut self,
        seat: SeatIndex,
        _: &mut T,
        moves: &[Decision<V>],
    ) -> Result<Option<Decision<V>>, T::Error> {
        let at = (self.pick)(self.seats.len(), moves.len());
        self.seats.push(seat.0);
        Ok(Some(moves[at]))
    }
}

fn played(cap: usize, pick: fn(usize, usize) -> usize) -> (Verdict, Vec<u8>) {
    let mut player = Noting {
        seats: Vec::new(),
        pick,
    };
    let verdict = sts2_run::run::<V, SimTable<V>, _>((), &lobby(), &mut player, cap);
    (verdict, player.seats)
}

#[test]
fn the_seats_take_turns() {
    let (verdict, seats) = played(6, |_, _| 0);
    assert_eq!(verdict.end, End::Capped, "{verdict}");
    assert_eq!(seats, [0, 1, 0, 1, 0, 1]);
}

/// A seat waits once it ends the turn or votes, until the others catch up.
/// It waits while a move of its own is on its way too. A question put to it is
/// answered whatever else it did.
#[test]
fn a_seat_that_ended_its_turn_or_voted_waits() {
    let mut table = SimTable::<V>::begin((), &lobby()).expect("the run begins");
    let mut seated = Seated {
        snapshot: table.snapshot(SeatIndex(0)).expect("seat 0 is seen"),
        peers: Peers {
            seats: vec![Peer::default(); 2],
            awaiting: Vec::new(),
            in_flight: false,
            still: true,
            links: vec![Default::default(); 2],
        },
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

/// Hunted seed 1 of v0.111.0, which two players carrying the hunt's relics
/// play to a loss.
#[test]
fn the_run_is_over_once_it_is_over_on_every_seat() {
    type W = sts2_content::v0_111_0::V0_111_0;
    let player = |character| Player { character, wins: 0 };
    let start = Start::<W> {
        seed: Seed::new("C000000001").expect("a seed"),
        players: vec![
            player(<W as sts2_core::ContentVersion>::Character::Ironclad),
            player(<W as sts2_core::ContentVersion>::Character::Silent),
        ],
        ascension: <W as sts2_core::ContentVersion>::AscensionLevel::None,
        modifiers: Vec::new(),
    };
    let mut table =
        sts2_run::Carried::<SimTable<W>>::begin((), &start).expect("the run begins and carries");
    let mut player = sts2_run::Random::for_seed("C000000001");
    let verdict = sts2_run::play_at(&mut table, &mut player, 20_000);
    assert!(
        matches!(verdict.end, End::Won | End::Lost),
        "both runs end: {verdict}"
    );
    for seat in SeatIndex::all(2) {
        let seen = table.snapshot(seat).expect("each seat is seen");
        assert!(
            seen.observation.run_over,
            "seat {} is still playing",
            seat.0
        );
    }
}

/// A solo run's table of one seat plays as the simulator does: the same
/// moves, to the same places, all the way to the end.
#[test]
fn a_table_of_one_plays_as_the_simulator() {
    let mut start = lobby();
    start.players.truncate(1);
    let mut sim = Sim::begin(&start).expect("the run begins");
    let mut table = SimTable::<V>::begin((), &start).expect("the run begins");
    for step in 0..20_000_usize {
        let seen = sim.observe().expect("the simulator observes");
        assert_eq!(
            table.snapshot(SeatIndex(0)).expect("the table answers"),
            seen,
            "step {step}"
        );
        if seen.observation.run_over {
            return;
        }
        let moves = &seen.observation.moves;
        let decision = moves[step.wrapping_mul(7919) % moves.len()];
        sim.play(&decision)
            .expect("no fault")
            .expect("a listed move is accepted");
        table
            .play(SeatIndex(0), &decision)
            .expect("no fault")
            .expect("a listed move is accepted");
    }
    panic!("the run never ended");
}
