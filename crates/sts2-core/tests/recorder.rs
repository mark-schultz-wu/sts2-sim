//! What the recorder writes down, and what it leaves out.
//!
//! An integration test for the same reason `digest.rs` is one. The claims are
//! about a run of a real build, and naming one means depending on
//! `sts2-content`, which depends back on this crate.
//!
//! The game underneath is a stub, and that is the point. Every claim here is
//! about the wrapper's rule, not about any implementation. A stub is the only
//! way to hold "the game refused it" fixed while asking what the tape did.

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{
    Answered, Applied, Construct, ContentVersion, Coord, Decision, Gift, Keyed, Map, Observation,
    Play, Reason, Recorder, Recording, Refusal, Setup, Snapshot, Start, Step, Table, Tape,
};

type V = V0_107_1;
type Encounter = <V as ContentVersion>::Encounter;
type Relic = <V as ContentVersion>::Relic;

/// The smallest state a reply can carry. Nothing here is asserted on, because
/// the recorder never looks inside an `Applied`, only at whether there was one.
fn nowhere() -> Applied<V> {
    Applied {
        observation: Observation {
            act: 0,
            floor: 1,
            hp: 80,
            max_hp: 80,
            alive: true,
            run_over: false,
            victory: None,
            gold: 99,
            relics: Vec::new(),
            potions: Vec::new(),
            potion_slots: 3,
            deck: Vec::new(),
            in_combat: false,
            map: Map {
                at: Some(Coord::new(3, 11)),
                visited: Vec::new(),
                travelable: Vec::new(),
                points: [].into(),
                proceed: false,
            },
            rng: sts2_core::Positions {
                run: Keyed::new(),
                player: Keyed::new(),
            },
            room: None,
            combat: None,
            moves: Vec::new(),
        },
        asking: None,
        resolved: true,
    }
}

/// What a stub game is told to answer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Answers {
    Accept,
    /// The game said no. A legitimate answer, and not an error.
    Refuse,
    /// The exchange did not happen: a socket died, a reply did not parse.
    Fault,
}

struct Stub {
    answers: Answers,
    /// Every decision it was asked, so a claim can say the move still reached
    /// the game even when nothing was written down.
    asked: Vec<Step<V>>,
}

impl Stub {
    fn new(answers: Answers) -> Stub {
        Stub {
            answers,
            asked: Vec::new(),
        }
    }

    fn answer(&mut self) -> Result<Answered<V>, &'static str> {
        match self.answers {
            Answers::Accept => Ok(Ok(nowhere())),
            Answers::Refuse => Ok(Err(Refusal {
                what: "play_card".into(),
                reason: Reason::NotInCombat,
            })),
            Answers::Fault => Err("the game stopped answering"),
        }
    }
}

impl Play<V> for Stub {
    type Error = &'static str;
    type Host = Stub;

    /// The stub is what it is before a run starts, so beginning hands it back.
    fn begin(host: Self::Host, _: &Start<V>) -> Result<Self, Self::Error> {
        Ok(host)
    }

    fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Self::Error> {
        self.asked.push(Step::Decision(*decision));
        self.answer()
    }

    fn observe(&mut self) -> Result<Snapshot<V>, Self::Error> {
        match self.answer()? {
            Ok(applied) => Ok(applied.snapshot()),
            Err(_) => Err("the stub does not refuse an observation"),
        }
    }
}

impl Construct<V> for Stub {
    fn setup(&mut self, step: &Setup<V>) -> Result<Answered<V>, Self::Error> {
        self.asked.push(Step::Setup(step.clone()));
        self.answer()
    }
}

/// A run for a tape to name. Nothing here reads it, and the stub answers
/// alike either way.
fn started() -> Start<V> {
    Start {
        seed: sts2_core::Seed::new("SP1RE1").expect("a seed"),
        character: <V as ContentVersion>::Character::ALL[0],
        ascension: <V as ContentVersion>::AscensionLevel::ALL[0],
        modifiers: vec![],
    }
}

fn recorder(answers: Answers) -> Recorder<V, Stub, Recording<V>> {
    Recorder::new(Stub::new(answers), Recording::new(started()))
}

fn nth<T: Table + Copy>(n: usize) -> T {
    <T as Table>::ALL[n]
}

fn play(index: u8) -> Decision<V> {
    Decision::PlayCard {
        index: sts2_core::fixture::hand(index),
        target: None,
    }
}

/// The exchange happened and the game took it.
///
/// Both halves asserted rather than the reply discarded. A `Result` of a
/// `Result` dropped on the floor is exactly how a refusal passes for a move.
/// Half these tests are about telling those two apart.
fn took(answered: Result<Answered<V>, &'static str>) {
    let applied = answered.expect("the stub answers");
    assert!(applied.is_ok(), "the stub was told to accept");
}

/// Observing has one answer, so there is nothing to unwrap twice.
fn saw(standing: Result<Snapshot<V>, &'static str>) {
    standing.expect("the stub answers");
}

fn steps(taped: &Recorder<V, Stub, Recording<V>>) -> Vec<Step<V>> {
    taped.tape().map(|r| r.steps().to_vec()).unwrap_or_default()
}

#[test]
fn every_decision_the_game_took_is_on_the_tape_in_order() {
    let mut taped = recorder(Answers::Accept);

    for index in 0..3 {
        took(taped.play(&play(index)));
    }

    assert_eq!(
        steps(&taped),
        vec![
            Step::Decision(play(0)),
            Step::Decision(play(1)),
            Step::Decision(play(2)),
        ]
    );
}

/// A refused move did not happen. Writing it down makes the tape replay as a
/// different run. Refused again, which is noise, or accepted by whatever it is
/// replayed into, which is worse.
#[test]
fn a_refused_move_is_not_a_step_in_the_run() {
    let mut taped = recorder(Answers::Refuse);

    let answered = taped.play(&play(0)).expect("a refusal is still a reply");
    assert!(answered.is_err(), "the stub was told to refuse");

    assert!(steps(&taped).is_empty(), "{:?}", steps(&taped));
    // And it still reached the game: the recorder is in the way of the tape,
    // not in the way of the move.
    assert_eq!(taped.inner().asked.len(), 1);
}

/// A fault is not a step either, and for a stronger reason: nobody knows
/// whether the game applied it.
#[test]
fn a_move_that_never_landed_is_not_a_step_in_the_run() {
    let mut taped = recorder(Answers::Fault);

    assert!(taped.play(&play(0)).is_err());
    assert!(steps(&taped).is_empty(), "{:?}", steps(&taped));
}

/// Observing moves nothing, so replaying it only makes a replay slower.
#[test]
fn looking_at_the_run_is_not_a_step_in_it() {
    let mut taped = recorder(Answers::Accept);

    saw(taped.observe());
    took(taped.play(&play(0)));
    saw(taped.observe());

    assert_eq!(steps(&taped), vec![Step::Decision(play(0))]);
}

/// A run reached by `fight` cannot be reproduced from a tape that dropped it.
#[test]
fn constructing_is_recorded_beside_playing() {
    let mut taped = recorder(Answers::Accept);

    let fight = Setup::Fight {
        encounter: nth::<Encounter>(0),
    };
    let gift = Setup::Grant(Gift::Relic(nth::<Relic>(0)));

    took(taped.setup(&fight));
    took(taped.play(&play(0)));
    took(taped.setup(&gift));

    assert_eq!(
        steps(&taped),
        vec![
            Step::Setup(fight),
            Step::Decision(play(0)),
            Step::Setup(gift),
        ]
    );
}

// ---------------------------------------------------------------------------
// A tape that fails.
// ---------------------------------------------------------------------------

/// A tape that takes `writes` steps and then refuses every one after.
struct Failing {
    writes: usize,
    taken: Vec<Step<V>>,
}

impl Tape<V> for Failing {
    type Error = String;

    fn record(&mut self, step: &Step<V>) -> Result<(), String> {
        if self.taken.len() >= self.writes {
            return Err("the disk filled up".into());
        }
        self.taken.push(step.clone());
        Ok(())
    }
}

/// The move happened. Turning a write failure into an error leaves the caller
/// believing a decision it already made did not take place.
#[test]
fn a_tape_that_fails_stops_and_says_so_without_failing_the_move() {
    let mut taped = Recorder::new(
        Stub::new(Answers::Accept),
        Failing {
            writes: 1,
            taken: Vec::new(),
        },
    );

    took(taped.play(&play(0)));
    assert!(taped.recording(), "one write was allowed");
    assert_eq!(taped.fault(), None);

    // The second write fails. The move still succeeds.
    let answered = taped.play(&play(1)).expect("the stub answers");
    assert!(answered.is_ok(), "the move was refused by a broken tape");

    assert!(!taped.recording(), "the recording should have stopped");
    assert_eq!(taped.fault(), Some("the disk filled up"));

    // And the game keeps working with nothing being written.
    took(taped.play(&play(2)));
    assert_eq!(taped.inner().asked.len(), 3);
    assert!(taped.fault().is_some(), "the reason is not forgotten");
}

/// Stopping by hand hands the tape back, so a caller can write it out or
/// compare it. What was recorded is not lost by stopping.
#[test]
fn stopping_hands_the_tape_back() {
    let mut taped = recorder(Answers::Accept);
    took(taped.play(&play(0)));

    let tape = taped.stop().expect("it was recording");
    assert_eq!(tape.steps(), [Step::Decision(play(0))]);

    assert!(!taped.recording());
    // Stopped, not broken: nothing went wrong, so there is nothing to report.
    assert_eq!(taped.fault(), None);

    took(taped.play(&play(1)));
    assert_eq!(taped.inner().asked.len(), 2, "the game still plays");
}
