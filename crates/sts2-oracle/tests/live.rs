//! Tests that launch the real game.
//!
//! Each one costs a process and ~3s of startup, and they need the mod deployed
//! into the game directory, so they are `#[ignore]`d and run by `just live`.
//! Everything checkable without the game lives in unit tests beside the code.
//!
//! # Why these are serial
//!
//! Every game process shares one profile directory, and a driven run writes to
//! `saves/progress.save` inside it — `RelicCmd.Obtain` calls
//! `SaveManager.MarkRelicAsSeen`, and fighting an enemy records a discovery.
//! Two runs at once would race on that file, and D1 treats profile residue as a
//! determinism concern rather than a tidiness one, so it is not something to
//! race on and hope.
//!
//! `#[serial(game)]` puts that constraint on the tests that have it instead of
//! on whoever invokes cargo. Anything not in the `game` group — the unit tests,
//! and the handshake tests that only bind loopback ports — still runs in
//! parallel, so `cargo test --include-ignored` is safe with no extra flags.

use serde_json::Value;
use serial_test::serial;
use std::time::{Duration, Instant};
use sts2_oracle::{Answer, Command, Config, Error, Oracle, PROTOCOL_VERSION, Reply, Response};

/// The seed the mod itself defaults to. Named here so the determinism test is
/// obviously asking both runs the same question.
const SEED: &str = "ORACLE1";

/// The encounter the scenario tests jump into. Two monsters, one of which the
/// encounter picks with its own RNG — which is what makes it worth checking.
const ENCOUNTER: &str = "BOWLBUGS_WEAK";

fn launch() -> Oracle {
    let dir = sts2_oracle::game_dir_from_env()
        .expect("set STS2_GAME_DIR to the game directory (`just live` does this)");
    Oracle::launch(Config::new(dir)).expect("the game should launch and connect back")
}

/// The parts of an observation these tests look at.
///
/// `Response::Observation` is a struct variant, so there is no payload type to
/// hand between helpers; this carries the few fields they share.
struct Seen {
    resolved: bool,
    digest: String,
    state: Value,
    game_errors: Option<Vec<String>>,
}

fn observation(reply: Reply) -> Seen {
    match reply.expect("expected the command to be accepted") {
        Response::Observation {
            resolved,
            digest,
            state,
            game_errors,
            ..
        } => Seen {
            resolved,
            digest,
            state,
            game_errors,
        },
        other => panic!("expected an observation, got {other:?}"),
    }
}

fn start(oracle: &mut Oracle) -> Seen {
    let reply = oracle
        .send(&Command::Start {
            seed: Some(SEED.into()),
            character: None,
            ascension: 0,
            modifiers: vec![],
        })
        .expect("start should not fail at the transport level");
    observation(reply)
}

#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn the_greeting_matches_what_this_client_speaks() {
    let oracle = launch();
    let greeting = oracle.greeting();

    assert_eq!(greeting.protocol, PROTOCOL_VERSION);
    assert_eq!(greeting.game, "v0.107.1");
    assert_eq!(greeting.assembly_hash, 1_692_500_715);

    // Every command this crate can build must be one the oracle accepts, or the
    // enum has drifted from the protocol it claims to implement.
    //
    // The oracle also offers `observe`, which this client deliberately does not
    // build: it reports state without advancing, and every response that reports
    // state already carries it.
    for cmd in [
        "start",
        "fight",
        "travel",
        "play_card",
        "end_turn",
        "answer",
        "grant",
        "enter_room",
        "choose_option",
        "buy",
        "quit",
    ] {
        assert!(
            greeting.commands.iter().any(|c| c == cmd),
            "oracle does not offer '{cmd}'; it offers {:?}",
            greeting.commands
        );
    }
}

#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn every_observation_carries_state() {
    let mut oracle = launch();
    let observed = start(&mut oracle);

    assert!(observed.resolved);
    assert!(!observed.digest.is_empty());
    assert!(
        observed.state.is_object(),
        "expected a state object, got {}",
        observed.state
    );
    assert_eq!(observed.game_errors, None);
}

/// D1, and the property the whole project rests on: the same seed has to produce
/// the same state. Two processes rather than two runs in one, so nothing carried
/// over in memory can make them agree.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn the_same_seed_gives_the_same_digest() {
    let first = {
        let mut oracle = launch();
        start(&mut oracle).digest
    };
    let second = {
        let mut oracle = launch();
        start(&mut oracle).digest
    };

    assert_eq!(first, second, "same seed, different state");
}

/// The encounter RNG patch, measured end to end.
///
/// `sts2_determinism` patches `DebugRandomizeRng` so an encounter is seeded from
/// the run rather than the clock, and `sts2_determinism_test` asserts the seed
/// directly. This asserts the consequence: the same seed and the same encounter
/// produce the same fight, in two processes, more than a wall-clock second
/// apart — which is the granularity the clock seeding used, and so the interval
/// over which an unpatched build would disagree with itself.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn the_same_encounter_gives_the_same_fight_across_a_second() {
    fn build() -> (String, Instant) {
        let mut oracle = launch();
        start(&mut oracle);
        let reply = oracle
            .send(&Command::Fight {
                encounter: ENCOUNTER.into(),
            })
            .expect("fight should not fail at the transport level");
        (observation(reply).digest, Instant::now())
    }

    let (first, at) = build();
    let (second, then) = build();

    // Without this the test could quietly become vacuous if launches ever got
    // fast enough for both fights to land in the same clock second.
    let gap = then.duration_since(at);
    assert!(
        gap > Duration::from_secs(1),
        "both fights landed inside one clock second ({gap:?}), so this proves nothing"
    );
    assert_eq!(first, second, "same seed and encounter, different fight");
}

/// A refused command is data, not a broken session. If this ever starts failing
/// by timing out rather than by assertion, the refusal path has stopped
/// replying at all.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn a_refusal_leaves_the_session_usable() {
    let mut oracle = launch();
    start(&mut oracle);

    // Nothing is pending, so this is refused — and the refusal names the command.
    let refusal = oracle
        .send(&Command::Answer(Answer::Cards(vec![0])))
        .expect("a refusal is still a reply, not a transport failure")
        .expect_err("nothing is pending, so this must be refused");
    assert_eq!(refusal.cmd, "answer");
    assert!(!refusal.message.is_empty());

    // The session survived it: a real command still works, and advances.
    let after = start(&mut oracle);
    assert!(after.state.is_object());
}

/// A healthy run reports no game errors at all.
///
/// It used to report one on every card played: the model layer calls
/// presentation inline, `CreatureCmd` asks for a screen shake when damage lands,
/// and headless has no screen to shake. `sts2_headless` patches that out, which
/// is what makes a non-empty `gameErrors` mean something again — the field is
/// how a half-completed action is caught, and it cannot do that while it fires
/// on every turn.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn an_ordinary_card_play_logs_no_game_errors() {
    let mut oracle = launch();
    start(&mut oracle);

    let fought = observation(
        oracle
            .send(&Command::Fight {
                encounter: ENCOUNTER.into(),
            })
            .expect("fight should not fail at the transport level"),
    );
    assert_eq!(fought.game_errors, None, "entering a fight logged errors");

    // Index 2 is the Strike in this encounter's opening hand; creature 1 is the
    // Bowlbug Rock. Both come from the observation above.
    let played = observation(
        oracle
            .send(&Command::PlayCard {
                index: 2,
                target: Some(1),
            })
            .expect("play_card should not fail at the transport level"),
    );
    assert_eq!(played.game_errors, None, "playing a card logged errors");
    assert!(played.resolved);
}

/// `Unassigned` is a `RoomType` the enum carries and nothing can be jumped to —
/// it is the "no map point" placeholder `fight` lands on. It used to parse, be
/// offered in the refusal message, and be handed to `EnterRoomDebug`.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn a_room_type_that_is_not_a_room_is_refused() {
    let mut oracle = launch();
    start(&mut oracle);

    let refusal = oracle
        .send(&Command::EnterRoom {
            room: "Unassigned".into(),
        })
        .expect("a refusal is still a reply")
        .expect_err("Unassigned is not somewhere to jump to");

    assert!(
        !refusal.message.contains("Unassigned"),
        "refused, but still offers it: {}",
        refusal.message
    );
    // The message earns its keep by naming what would have worked.
    assert!(
        refusal.message.contains("Shop"),
        "should list the types that do work: {}",
        refusal.message
    );
}

/// Dropping the client has to take the game with it, or a test run leaves
/// headless games behind.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn dropping_the_client_stops_the_game() {
    let pid = {
        let oracle = launch();
        let pid = oracle.pid();
        drop(oracle);
        pid
    };

    // Give the OS a moment to finish tearing it down before looking. `reap`
    // waits, so the entry is gone rather than left as a zombie.
    std::thread::sleep(Duration::from_millis(500));

    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "game process {pid} survived the client that launched it"
    );
}

/// The launch path has to fail loudly rather than hang when the game is not
/// where it was said to be.
#[test]
fn a_bad_game_dir_fails_without_the_game() {
    let err = Oracle::launch(Config::new("/definitely/not/the/game"))
        .map(|_| ())
        .expect_err("should refuse");
    assert!(matches!(err, Error::GameNotFound(_)), "got {err:?}");
}
