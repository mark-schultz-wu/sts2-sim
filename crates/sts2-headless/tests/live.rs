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

use nonmax::NonMaxU32;
use serde_json::Value;
use serde_json::json;
use serial_test::serial;
use std::time::{Duration, Instant};
use sts2_content::ContentVersion;
use sts2_content::v0_107_1::V0_107_1;
use sts2_headless::{
    Answer, Command, Config, Error, Game, PROTOCOL_VERSION, Question, Reply, Response,
};
use sts2_rng::Seed;

/// The build these tests drive.
///
/// Every other content type is reached through [`ContentVersion`] rather than
/// out of the version module, so pointing this at another build is a one-line
/// change. Naming `v0_107_1::characters::Character` directly would put the build
/// back into every call site, which is what the parameter exists to remove.
type V = V0_107_1;
type AscensionLevel = <V as ContentVersion>::AscensionLevel;
type Character = <V as ContentVersion>::Character;
type Encounter = <V as ContentVersion>::Encounter;
type RoomType = <V as ContentVersion>::RoomType;

/// The seed the mod itself defaults to. Named here so the determinism test is
/// obviously asking both runs the same question.
const SEED: &str = "ORACLE1";

/// The encounter the scenario tests jump into. Two monsters, one of which the
/// encounter picks with its own RNG — which is what makes it worth checking.
const ENCOUNTER: Encounter = Encounter::BowlbugsWeak;

fn launch() -> Game<V> {
    let dir = sts2_headless::game_dir_from_env()
        .expect("set STS2_GAME_DIR to the game directory (`just live` does this)");
    Game::<V>::launch(Config::new(dir)).expect("the game should launch and connect back")
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

fn start(game: &mut Game<V>) -> Seen {
    let reply = game
        .send(&Command::Start {
            seed: Seed::new(SEED).expect("a seed"),
            character: Character::Ironclad,
            ascension: AscensionLevel::None,
            modifiers: vec![],
        })
        .expect("start should not fail at the transport level");
    observation(reply)
}

#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn the_greeting_matches_what_this_client_speaks() {
    let game = launch();
    let greeting = game.greeting();

    assert_eq!(greeting.protocol, PROTOCOL_VERSION);
    assert_eq!(greeting.game, "v0.107.1");
    assert_eq!(greeting.assembly_hash, 1_692_500_715);

    // Every command this crate can build must be one the game accepts, or the
    // enum has drifted from the protocol it claims to implement.
    //
    // The game also offers `observe`, which this client deliberately does not
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
            "game does not offer '{cmd}'; it offers {:?}",
            greeting.commands
        );
    }
}

#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn every_observation_carries_state() {
    let mut game = launch();
    let observed = start(&mut game);

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
        let mut game = launch();
        start(&mut game).digest
    };
    let second = {
        let mut game = launch();
        start(&mut game).digest
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
        let mut game = launch();
        start(&mut game);
        let reply = game
            .send(&Command::Fight {
                encounter: ENCOUNTER,
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
    let mut game = launch();
    start(&mut game);

    // Nothing is pending, so this is refused — and the refusal names the command.
    let refusal = game
        .send(&Command::Answer(Answer::Cards(vec![0])))
        .expect("a refusal is still a reply, not a transport failure")
        .expect_err("nothing is pending, so this must be refused");
    assert_eq!(refusal.cmd, "answer");
    assert!(!refusal.message.is_empty());

    // The session survived it: a real command still works, and advances.
    let after = start(&mut game);
    assert!(after.state.is_object());
}

/// Every way a command can be turned down, in one launch.
///
/// The code review that prompted this found three bugs in paths nothing ran:
/// the refusal to start, the exceptions around `Driver.Run`'s setup, and two
/// unguarded dereferences. Tests that only walk the happy path leave the
/// failure handling to be discovered in production, and here "production" is a
/// divergence report nobody can explain.
///
/// One launch for the lot, because a launch costs ~3s and an assertion costs
/// nothing. Every one of these is refused from the same opening state, so no
/// case can disturb another — which the selftest suite learned the hard way.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn illegal_commands_are_refused_and_leave_the_session_alive() {
    /// Asserts a command is turned down, and turned down for the stated reason.
    ///
    /// The reason matters as much as the refusal: a command refused because the
    /// game misunderstood it looks identical to one refused correctly, and
    /// only the message tells them apart.
    /// The same check for a command the typed API cannot build.
    ///
    /// Only for ids the content tables deliberately do not have — a name the
    /// game should refuse, or a `RoomType` codegen drops. A command that ought
    /// to work and cannot be built is a gap in the tables, not a reason to
    /// reach for this.
    fn refused_raw(game: &mut Game<V>, command: serde_json::Value, expect: &str) {
        let refusal = game
            .send_raw(&command)
            .expect("a refusal is still a reply, not a transport failure")
            .expect_err(&format!("{command} should have been refused"));
        assert!(
            refusal
                .message
                .to_lowercase()
                .contains(&expect.to_lowercase()),
            "{command} was refused, but not for the stated reason:\n  \
             wanted something mentioning {expect:?}\n  got {:?}",
            refusal.message
        );
    }

    fn refused(game: &mut Game<V>, command: Command<V>, expect: &str) {
        let refusal = game
            .send(&command)
            .expect("a refusal is still a reply, not a transport failure")
            .expect_err(&format!("{command:?} should have been refused"));
        assert!(
            refusal
                .message
                .to_lowercase()
                .contains(&expect.to_lowercase()),
            "{command:?} was refused, but not for the stated reason:\n  \
             wanted something mentioning {expect:?}\n  got {:?}",
            refusal.message
        );
    }

    let mut game = launch();
    start(&mut game);

    // Out of combat, so nothing that needs one may proceed.
    refused(
        &mut game,
        Command::PlayCard {
            index: 0,
            target: None,
        },
        "not in combat",
    );
    refused(&mut game, Command::EndTurn, "not in combat");
    refused(&mut game, Command::Buy { index: 0 }, "not in a shop");

    // Malformed in themselves.
    refused(
        &mut game,
        Command::Grant {
            card: None,
            relic: None,
        },
        "requires",
    );
    refused(
        &mut game,
        Command::Answer(Answer::Skip),
        "nothing is pending",
    );

    // Names the game does not know. The message comes from the game rather than
    // from us, which is the point of asking it instead of guessing.
    // Names the tables do not have, so they cannot be built as a `Command` at
    // all — which is the point of the typed API, and why the refusal path has
    // its own door.
    refused_raw(
        &mut game,
        json!({"cmd": "grant", "relic": "NOT_A_REAL_RELIC"}),
        "NOT_A_REAL_RELIC",
    );
    refused_raw(
        &mut game,
        json!({"cmd": "fight", "encounter": "NOT_A_REAL_ENCOUNTER"}),
        "not found",
    );

    // Legal shape, illegal move: the refusal lists what would have worked.
    refused(
        &mut game,
        Command::Travel { col: 99, row: 99 },
        "travelable",
    );

    // Eight refusals later the session still advances. `start` rather than
    // `observe`, which this client does not build — a command that changes
    // something is a better proof of life than one that only reports.
    let after = start(&mut game);
    assert!(after.state.is_object());
    assert_eq!(after.game_errors, None, "a refusal should log nothing");
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
    let mut game = launch();
    start(&mut game);

    let fought = observation(
        game.send(&Command::Fight {
            encounter: ENCOUNTER,
        })
        .expect("fight should not fail at the transport level"),
    );
    assert_eq!(fought.game_errors, None, "entering a fight logged errors");

    // Index 0 is the Strike in this encounter's opening hand — which is
    // `["STRIKE_IRONCLAD", "BASH", "DEFEND_IRONCLAD", ...]` — and creature 1 is
    // the Bowlbug Rock. Both come from the observation above.
    //
    // It used to be index 2, and moved when `Seed` started folding `ORACLE1` to
    // `0RACLE1` the way the game does. That is the whole point of the type: the
    // old spelling was seeding a run no player could reach by typing it.
    let played = observation(
        game.send(&Command::PlayCard {
            index: 0,
            target: NonMaxU32::new(1),
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
    let mut game = launch();
    start(&mut game);

    // `Unassigned` is a real `RoomType` that codegen deliberately drops, so the
    // typed API cannot name it and this has to go raw.
    let refusal = game
        .send_raw(&json!({"cmd": "enter_room", "type": "Unassigned"}))
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
        let game = launch();
        let pid = game.pid();
        drop(game);
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

/// Refusing to start must end the process, not sit there.
///
/// This is the bug a code review found and nothing covered: `Driver.Run` called
/// `Environment.Exit(1)`, which does not work from Godot's main loop, so the
/// process logged "Exiting." and ran until something external killed it. The
/// branch had never been taken, because the only way to reach it is to break
/// determinism — and it stopped being broken the day the exit was added.
///
/// Launched directly rather than through [`Game`], because the client always
/// supplies `LD_PRELOAD` and the point is to withhold it: without libgcc,
/// Harmony cannot patch, `sts2_determinism` stays inert, and the driver refuses.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn refusing_to_start_exits_rather_than_hanging() {
    let dir = sts2_headless::game_dir_from_env().expect("STS2_GAME_DIR");
    let began = Instant::now();

    let status = std::process::Command::new(dir.join("SlayTheSpire2"))
        .current_dir(&dir)
        .args(["--headless", "--force-steam=off"])
        .env("STS2_HEADLESS_MODE", "selftest")
        .env_remove("LD_PRELOAD")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("the game should launch");

    let took = began.elapsed();
    assert!(
        took < Duration::from_secs(60),
        "took {took:?} — the refusal is hanging again"
    );
    assert_eq!(
        status.code(),
        Some(1),
        "a driver that refuses to start must exit non-zero"
    );
}

/// The launch path has to fail loudly rather than hang when the game is not
/// where it was said to be.
#[test]
fn a_bad_game_dir_fails_without_the_game() {
    let err = Game::<V>::launch(Config::new("/definitely/not/the/game"))
        .map(|_| ())
        .expect_err("should refuse");
    assert!(matches!(err, Error::GameNotFound(_)), "got {err:?}");
}

/// A rest site option that asks a question must not deadlock the driver.
///
/// This is the one class of bug only a live test can catch. The C# selftest
/// calls `Driver` directly and never touches `Protocol`, so it exercises rest
/// sites happily while the *protocol* path is wedged — which is exactly what
/// happened: 201 selftest assertions passed against a `choose_option` that
/// never replied.
///
/// The mechanism, since a future edit could reintroduce it: the handler used to
/// `await sync.ChooseLocalOption(index)`. For SMITH that awaits
/// `CardSelectCmd.FromDeckForUpgrade`, which parks a question for the client —
/// and the serve loop cannot read the client's answer until the handler returns.
/// The handler was waiting for something only its own return could produce.
/// Measured before the fix: no reply, client timed out at 20s.
///
/// HEAL is deliberately not the case under test. It resolves by itself, so it
/// would pass against the broken code and prove nothing.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn a_rest_site_option_that_asks_a_question_still_replies() {
    let mut game = launch();
    start(&mut game);

    let seen = observation(
        game.send(&Command::EnterRoom {
            room: RoomType::RestSite,
        })
        .expect("entering a rest site should not fail at the transport level"),
    );

    let smith = seen.state["room"]["options"]
        .as_array()
        .expect("a rest site reports its options")
        .iter()
        .find(|o| o["id"] == "SMITH")
        .and_then(|o| o["index"].as_u64())
        .expect("a fresh rest site offers SMITH");

    // The assertion is that this returns at all. A transport error here is the
    // deadlock; the command timeout is what turns the hang into a failure.
    let reply = game
        .send(&Command::ChooseOption { index: smith as u8 })
        .expect("choose_option on SMITH must reply, not hang");

    match reply.expect("SMITH should be accepted") {
        Response::Pending { question, .. } => match question {
            Question::SelectCards { min, options, .. } => {
                assert_eq!(min, 1, "smithing upgrades exactly one card");
                assert!(!options.is_empty(), "the deck should be offered");
            }
            other => panic!("expected a card selection, got {other:?}"),
        },
        other => panic!("SMITH should park a question, got {other:?}"),
    }

    // Returning is not the same as working: answer it and check the run moved.
    let after = observation(
        game.send(&Command::Answer(Answer::Cards(vec![0])))
            .expect("answering should not fail at the transport level"),
    );

    let upgraded = after.state["deck"]
        .as_array()
        .expect("the deck is reported")
        .iter()
        .filter_map(|c| c.as_str())
        // `Ident` packs the upgrade level into the id, as a `+N` suffix.
        .filter(|s| s.contains('+'))
        .count();
    assert_eq!(
        upgraded, 1,
        "smithing upgrades exactly one card in the deck"
    );
}
