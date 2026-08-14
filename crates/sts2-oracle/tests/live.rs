//! Tests that launch the real game.
//!
//! Each one costs a process and ~10s of startup, and they need the mod deployed
//! into the game directory, so they are `#[ignore]`d and run by `just live`.
//! Everything checkable without the game lives in unit tests beside the code.

use std::time::Duration;
use sts2_oracle::{Command, Config, Error, Oracle, PROTOCOL_VERSION, Response};

/// The seed the mod itself defaults to. Named here so the determinism test is
/// obviously asking both runs the same question.
const SEED: &str = "ORACLE1";

fn config() -> Config {
    let dir = sts2_oracle::game_dir_from_env()
        .expect("set STS2_GAME_DIR to the game directory (`just live` does this)");
    Config::new(dir).full_observations(true)
}

fn launch() -> Oracle {
    Oracle::launch(config()).expect("the game should launch and connect back")
}

fn observation(reply: Response) -> sts2_oracle::Observation {
    match reply {
        Response::Observation(o) => o,
        other => panic!("expected an observation, got {other:?}"),
    }
}

fn start(oracle: &mut Oracle) -> sts2_oracle::Observation {
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
fn the_greeting_matches_what_this_client_speaks() {
    let oracle = launch();
    let hello = oracle.hello();

    assert_eq!(hello.protocol, PROTOCOL_VERSION);
    assert_eq!(hello.game, "v0.107.1");
    assert_eq!(hello.assembly_hash, 1_692_500_715);

    // Every command this crate can build must be one the oracle accepts, or the
    // enum has drifted from the protocol it claims to implement.
    for cmd in [
        "start",
        "enter_fight",
        "travel",
        "play_card",
        "end_turn",
        "answer",
        "grant",
        "enter_room",
        "choose_option",
        "buy",
        "observe",
        "quit",
    ] {
        assert!(
            hello.commands.iter().any(|c| c == cmd),
            "oracle does not offer '{cmd}'; it offers {:?}",
            hello.commands
        );
    }
}

#[test]
#[ignore = "launches the real game"]
fn a_full_observation_actually_carries_state() {
    let mut oracle = launch();
    let observed = start(&mut oracle);

    assert!(observed.resolved);
    assert!(!observed.digest.is_empty());
    assert!(
        observed.state.is_some(),
        "Config::full_observations should have put state on the wire"
    );
    assert_eq!(observed.game_errors, None);
}

/// R1, and the property the whole project rests on: the same seed has to produce
/// the same state. Two processes rather than two runs in one, so nothing carried
/// over in memory can make them agree.
#[test]
#[ignore = "launches the real game"]
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

/// A refused command is data, not a broken session. If this ever starts failing
/// by timing out rather than by assertion, the refusal path has stopped
/// replying at all.
#[test]
#[ignore = "launches the real game"]
fn a_refusal_leaves_the_session_usable() {
    let mut oracle = launch();
    start(&mut oracle);

    // Nothing is pending, so this is refused — and the refusal names the command.
    let reply = oracle
        .send(&Command::AnswerCards { cards: vec![0] })
        .expect("a refusal is still a reply");
    let Response::Error(refusal) = reply else {
        panic!("expected a refusal, got {reply:?}");
    };
    assert_eq!(refusal.cmd, "answer");
    assert!(!refusal.message.is_empty());

    // The session survived it.
    let after = observation(oracle.send(&Command::Observe).expect("observe should work"));
    assert!(after.state.is_some());
}

/// Dropping the client has to take the game with it, or a test run leaves
/// headless games behind.
#[test]
#[ignore = "launches the real game"]
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
