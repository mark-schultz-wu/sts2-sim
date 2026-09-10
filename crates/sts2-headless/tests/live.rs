//! Tests that launch the real game.
//!
//! Each one costs a process and ~3s of startup. They need the mod deployed into
//! the game directory, so they are `#[ignore]`d and run by `just live`.
//! Everything checkable without the game lives in unit tests beside the code.
//!
//! The tests that drive a run carry `#[serial(game)]`, because every game
//! process shares one profile directory. The crate README says why that
//! matters.

use serde_json::Value;
use serde_json::json;
use serial_test::serial;
use std::time::{Duration, Instant};
use sts2_content::v0_107_1::V0_107_1;
use sts2_core::ContentVersion;
use sts2_core::Seed;
use sts2_core::fixture::{at, belt, hand};
use sts2_core::{
    Answer, Answered, Construct, Coord, Decision, Digest, Gift, Play, Question, Reason, Recorder,
    Room, Setup, Start, Step, Stock, Tool,
};
use sts2_core::{Decided, Snapshot};
use sts2_headless::{Config, Error, MOD_IDENTITY, Oracle, Response, recording};

/// The build these tests drive.
///
/// Every other content type is reached through [`ContentVersion`] rather than
/// out of the version module, so pointing this at another build is a one-line
/// change. Naming `v0_107_1::characters::Character` directly puts the build back
/// into every call site.
type V = V0_107_1;
type AscensionLevel = <V as ContentVersion>::AscensionLevel;
type Character = <V as ContentVersion>::Character;
type Encounter = <V as ContentVersion>::Encounter;
type Potion = <V as ContentVersion>::Potion;
type Event = <V as ContentVersion>::Event;
type Relic = <V as ContentVersion>::Relic;
type Enchantment = <V as ContentVersion>::Enchantment;
type RoomType = <V as ContentVersion>::RoomType;
type RestSiteOption = <V as ContentVersion>::RestSiteOption;

/// The seed the mod itself defaults to. Named here so the determinism test is
/// obviously asking both runs the same question.
const SEED: &str = "0RACLE1";

/// The encounter the scenario tests jump into. Two monsters, one of which the
/// encounter picks with its own RNG. That is what makes it worth checking.
const ENCOUNTER: Encounter = Encounter::BowlbugsWeak;

fn launch() -> Oracle<V> {
    let dir = sts2_headless::game_dir_from_env()
        .expect("set STS2_GAME_DIR to the game directory (`just live` does this)");
    Oracle::<V>::launch(Config::new(dir)).expect("the game should launch and connect back")
}

/// The run these tests play.
fn opening() -> Start<V> {
    Start {
        seed: Seed::new(SEED).expect("a seed"),
        character: Character::Ironclad,
        ascension: AscensionLevel::None,
        modifiers: vec![],
    }
}

/// A game with the run already started, which is the only kind there is.
fn launched() -> Oracle<V> {
    let dir = sts2_headless::game_dir_from_env()
        .expect("set STS2_GAME_DIR to the game directory (`just live` does this)");
    Oracle::<V>::begin(Config::new(dir), &opening())
        .expect("the game should launch and start the run")
}

/// The parts of an answer these tests look at.
///
/// Both halves: what the game said, typed, and the envelope it said it in. The
/// envelope is here because these are the oracle's own tests. Several of them
/// are about the game's digest and the state as it wrote it, and neither
/// survives into [`Applied`].
struct Seen {
    snapshot: Snapshot<V>,
    /// Absent for a look, which decided nothing.
    decided: Option<Decided>,
    /// The digest the game computed, over its own state.
    reported: String,
    /// The state as the game wrote it.
    state: Value,
    /// What the game logged while running the command.
    errors: Vec<String>,
}

impl Seen {
    /// This crate's digest, over the typed observation.
    ///
    /// The one the differential test uses. It is defined over the type both
    /// implementations produce, rather than over a JSON string built in C#. See
    /// [`the_same_seed_gives_the_same_digest`] for what the pair catches that
    /// neither catches alone.
    fn digest(&self) -> Digest {
        self.snapshot.observation.digest()
    }

    /// What the game stopped to ask, for a command that parked it.
    fn asking(&self) -> Option<&Question<V>> {
        self.snapshot.asking.as_ref()
    }
}

/// One answer, with the envelope it arrived in.
///
/// Taken apart in two statements at every call site rather than one, because
/// the reply borrows the game mutably and the envelope borrows it again.
fn observation(game: &Oracle<V>, answered: Answered<V>) -> Seen {
    let applied = answered.expect("expected the command to be accepted");
    let (decided, snapshot) = applied.split();
    let response = game.reported().expect("a command was sent");

    Seen {
        snapshot,
        decided: Some(decided),
        reported: match response {
            Response::Observation { digest, .. } | Response::Pending { digest, .. } => {
                digest.clone()
            }
            Response::Bye => String::new(),
        },
        state: response.state().cloned().unwrap_or(Value::Null),
        errors: game.logged().to_vec(),
    }
}

/// One decision, sent and read.
fn played(game: &mut Oracle<V>, decision: &Decision<V>) -> Seen {
    let answered = game
        .play(decision)
        .expect("the command should not fail at the transport level");
    observation(game, answered)
}

/// One setup, sent and read.
fn constructed(game: &mut Oracle<V>, setup: &Setup<V>) -> Seen {
    let answered = game
        .setup(setup)
        .expect("the setup should not fail at the transport level");
    observation(game, answered)
}

/// One line of a recording, sent and read.
fn stepped(game: &mut Oracle<V>, step: &Step<V>) -> Seen {
    let answered = game
        .step(step)
        .expect("the step should not fail at the transport level");
    observation(game, answered)
}

/// Where a freshly started run is standing.
///
/// Starting is not a step any more, so this asks rather than sends. The run is
/// begun when the game is reached, and this reports where that left it.
fn start(game: &mut Oracle<V>) -> Seen {
    let snapshot = game
        .observe()
        .expect("observe should not fail at the transport level");
    looked(game, snapshot)
}

/// A look, which decided nothing and so has no [`Decided`].
fn looked(game: &Oracle<V>, snapshot: Snapshot<V>) -> Seen {
    let response = game.reported().expect("a command was sent");

    Seen {
        snapshot,
        decided: None,
        reported: match response {
            Response::Observation { digest, .. } | Response::Pending { digest, .. } => {
                digest.clone()
            }
            Response::Bye => String::new(),
        },
        state: response.state().cloned().unwrap_or(Value::Null),
        errors: game.logged().to_vec(),
    }
}

/// The belt, end to end: what it reports, what it refuses, and what drinking
/// one actually does to the fight.
///
/// Worth a live test rather than a fixture, because none of it is in a table.
/// `usable`, `reason` and `targets` are computed from `PotionUsage`,
/// `CombatManager` and `PotionModel.IsValidTarget`, and the prose comes out of
/// the localisation tables.
/// Whether the damage lands is the point of the command.

#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn a_potion_is_reported_refused_and_drunk() {
    let mut game = launch();
    start(&mut game);

    // Fire Potion is `CombatOnly` and `AnyEnemy`: out of combat it is refused,
    // in combat it must be pointed at somebody, and what it does is visible in
    // a creature's hit points.
    let held = constructed(&mut game, &Setup::Grant(Gift::Potion(Potion::FirePotion)));
    assert!(held.errors.is_empty(), "granting a potion logged errors");

    let [potion] = &held.snapshot.observation.potions[..] else {
        panic!(
            "one potion in the belt, got {:?}",
            held.snapshot.observation.potions
        );
    };
    assert_eq!(potion.potion, Potion::FirePotion);
    assert_eq!(
        potion.reason,
        Some(sts2_core::Undrinkable::OutOfCombat),
        "a combat-only potion out of combat"
    );
    assert!(
        potion.targets.is_empty(),
        "no combat, so no combat ids to name"
    );
    // The words the game shows, out of its own localisation table. Asserted as
    // present rather than by their text, which a content patch may reword.
    assert!(potion.title.is_some(), "the potion has a name");
    assert!(potion.text.is_some(), "and says what it does");

    // Every relic carries the same, and it is why the field is here rather than
    // in a table: `DynamicDescription` prints a relic's live counter.
    for relic in &held.snapshot.observation.relics {
        assert!(relic.title.is_some(), "{relic:?} has no name");
        assert!(relic.text.is_some(), "{relic:?} says nothing");
    }

    // Refused where the belt said it would be, and for the reason the belt
    // gave. The reason matters as much as the refusal: a client that read the
    // belt and asked anyway must be told the same word it was already holding.
    let refusal = game
        .play(&Decision::UsePotion {
            index: belt(0),
            target: None,
        })
        .expect("a refusal is still a reply, not a transport failure")
        .expect_err("a combat-only potion out of combat should be refused");
    assert_eq!(
        refusal.reason,
        Reason::Undrinkable {
            potion: Potion::FirePotion,
            why: sts2_core::Undrinkable::OutOfCombat,
        }
    );

    let fighting = constructed(
        &mut game,
        &Setup::Fight {
            encounter: ENCOUNTER,
        },
    );
    let combat = fighting
        .snapshot
        .observation
        .fight()
        .expect("a fight to drink into");

    let potion = &fighting.snapshot.observation.potions[0];
    assert_eq!(potion.reason, None, "in combat it can be drunk");
    assert!(
        !potion.targets.is_empty(),
        "an `AnyEnemy` potion names who it may be thrown at"
    );

    let at = potion.targets[0];
    let before = combat
        .creatures
        .iter()
        .find(|c| c.id == at)
        .map(|c| c.hp)
        .expect("the target is in the fight");

    let drunk = played(
        &mut game,
        &Decision::UsePotion {
            index: belt(0),
            target: Some(at),
        },
    );
    assert!(drunk.errors.is_empty(), "drinking logged errors");

    assert!(
        drunk.snapshot.observation.potions.is_empty(),
        "the potion is still in the belt"
    );

    let after = drunk
        .snapshot
        .observation
        .fight()
        .and_then(|c| c.creatures.iter().find(|c| c.id == at).map(|c| c.hp));
    // Absent means it died, which is also the potion having landed.
    if let Some(after) = after {
        assert!(
            after < before,
            "the potion was drunk at {at} and did nothing: {before} -> {after}"
        );
    }
}

/// The one event with a screen of its own, played through the wire.
///
/// `NCrystalSphereScreen.ShowScreen` instantiates a scene the headless build has
/// no cache entry for. Asking for it takes the process down without an
/// exception, and the log stops after `Asset not cached`. The fuzzer found it at
/// floor 29.
///
/// The screen is skipped and the clicks arrive as `divine` questions instead.
/// [`Setup::Event`] is what makes this 3 steps and not 515.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn the_crystal_sphere_is_divined_over_the_wire() {
    let mut game = launch();
    start(&mut game);

    let at_event = constructed(
        &mut game,
        &Setup::Event {
            event: Event::CrystalSphere,
        },
    );
    assert!(
        at_event.errors.is_empty(),
        "entering the event logged errors"
    );
    assert_eq!(
        at_event.state.pointer("/room/id").and_then(Value::as_str),
        Some("CRYSTAL_SPHERE"),
        "the run should be standing in the named event"
    );

    // Payment Plan, which costs a Debt rather than gold and gives six
    // divinations rather than three -- the longer of the two paths, so more of
    // the loop is exercised.
    let mut seen = played(&mut game, &Decision::ChooseOption { index: 1 });
    let mut spent = 0;

    while let Some(question) = seen.asking() {
        let answer = match question {
            Question::Divine {
                left,
                width,
                height,
                cleared,
                ..
            } => {
                assert!(*left > 0, "a divination was asked for with none left");
                assert_eq!((*width, *height), (11, 11), "the grid is 11x11");

                // Any still-hidden cell. Which one is worth taking is not
                // knowable -- that is what the minigame is -- so this takes the
                // first and asserts only that the question is answerable.
                let (x, y) = (0..*width)
                    .flat_map(|x| (0..*height).map(move |y| (x, y)))
                    .find(|(x, y)| !cleared.iter().any(|c| c.x == *x && c.y == *y))
                    .expect("a hidden cell to divine");

                spent += 1;
                assert!(spent <= 6, "Payment Plan gives six divinations, not more");

                Answer::Divine {
                    x,
                    y,
                    tool: Tool::Big,
                }
            }

            // What the divinations uncovered is paid out as an ordinary reward
            // screen, through `DoLocalCrystalSphereRewards`. Skipped rather
            // than taken: this test is about reaching the payout, and taking is
            // covered where rewards are. Whether one is offered at all depends
            // on what the cells held, so this is not asserted -- only that it
            // is answerable and does not derail the event.
            Question::Rewards { .. } => Answer::Skip,

            other => panic!("expected a divination or its payout, got {other:?}"),
        };

        seen = played(&mut game, &Decision::Answer(answer));
    }

    assert_eq!(spent, 6, "Payment Plan gives six divinations");

    let done = seen;
    assert!(done.errors.is_empty(), "divining logged errors");
    // The event is through: still the Crystal Sphere, but on its closing page
    // with nothing left to choose. Reaching this at all is the point -- the
    // process used to die at the option that starts the minigame.
    assert_eq!(
        done.state.pointer("/room/id").and_then(Value::as_str),
        Some("CRYSTAL_SPHERE")
    );
    assert_eq!(
        done.state
            .pointer("/room/finished")
            .and_then(Value::as_bool),
        Some(true),
        "the event should be finished once the minigame pays out"
    );
}

/// A divination off the grid is refused, and the question survives it.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn a_divination_off_the_grid_is_refused() {
    let mut game = launch();
    start(&mut game);
    game.setup(&Setup::Event {
        event: Event::CrystalSphere,
    })
    .expect("jumping to an event should not fail at the transport level")
    .expect("the event should be accepted");
    game.play(&Decision::ChooseOption { index: 0 })
        .expect("choosing an option should not fail at the transport level")
        .expect("the option should be accepted");

    let refusal = game
        .play(&Decision::Answer(Answer::Divine {
            x: 99,
            y: 0,
            tool: Tool::Small,
        }))
        .expect("a refusal is data, not a transport failure")
        .expect_err("a cell off the grid should be refused");
    assert!(
        matches!(refusal.reason, Reason::OffGrid { x: 99, y: 0, .. }),
        "the refusal should say which cell was off the grid: {refusal:?}"
    );

    // Still askable afterwards: a refused answer must not consume a divination.
    let after = game
        .play(&Decision::Answer(Answer::Divine {
            x: 5,
            y: 5,
            tool: Tool::Small,
        }))
        .expect("a divination should not fail at the transport level");
    assert!(after.is_ok(), "the question should survive a refusal");
}

#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn the_greeting_matches_what_this_client_speaks() {
    let game = launch();
    let greeting = game.greeting();

    // `launch` already refuses a mismatch, so reaching here proves the deployed
    // mod was built from the source this client was. Asserted anyway, because
    // that is the claim the test is named for.
    assert_eq!(greeting.identity.as_deref(), Some(MOD_IDENTITY));
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
        "proceed",
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

    assert_eq!(observed.decided, Some(Decided::Resolved));
    assert!(!observed.reported.is_empty());
    assert!(
        observed.state.is_object(),
        "expected a state object, got {}",
        observed.state
    );
    assert!(observed.errors.is_empty());
}

/// D1, and the property the whole project rests on: the same seed has to produce
/// the same state. Two processes rather than two runs in one, so nothing carried
/// over in memory can make them agree.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn the_same_seed_gives_the_same_digest() {
    let run = || {
        let mut game = launch();
        let seen = start(&mut game);
        (seen.reported.clone(), seen.digest())
    };

    let (first_in_c_sharp, first) = run();
    let (second_in_c_sharp, second) = run();

    assert_eq!(first, second, "same seed, different state");

    // Both digests, because the pair says something neither says alone. The
    // mod's is over the whole state it built; this crate's is over the fields
    // that survived into `Observation<V>`. Two runs the game calls different
    // and this crate calls the same is an observation missing something —
    // exactly the divergence a simulator would then be free to get wrong.
    assert_eq!(
        first_in_c_sharp == second_in_c_sharp,
        first == second,
        "the game and this crate disagree about whether these are the same run"
    );
}

/// The encounter RNG patch, measured end to end.
///
/// `sts2_determinism` patches `DebugRandomizeRng` so an encounter is seeded from
/// the run rather than the clock, and `sts2_determinism_test` asserts the seed
/// directly. This asserts the consequence: the same seed and encounter produce
/// the same fight in two processes, more than a wall-clock second apart. A
/// second is the granularity the clock seeding used.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn the_same_encounter_gives_the_same_fight_across_a_second() {
    fn build() -> (Digest, Instant) {
        let mut game = launch();
        start(&mut game);
        let reply = game
            .setup(&Setup::Fight {
                encounter: ENCOUNTER,
            })
            .expect("fight should not fail at the transport level");
        (observation(&game, reply).digest(), Instant::now())
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
        .play(&Decision::Answer(Answer::Cards(vec![0])))
        .expect("a refusal is still a reply, not a transport failure")
        .expect_err("nothing is pending, so this must be refused");
    assert_eq!(refusal.what, "answer");
    assert_eq!(refusal.reason, Reason::NothingPending);

    // The session survived it: a real command still works, and advances.
    let after = start(&mut game);
    assert!(after.state.is_object());
}

/// Every way a command can be turned down, in one launch.
///
/// The code review that prompted this found three bugs in paths nothing ran:
/// the refusal to start, the exceptions around `Driver.Run`'s setup, and two
/// unguarded dereferences.
///
/// One launch for the lot, because a launch costs ~3s and an assertion costs
/// nothing. Every case is refused from the same opening state, so none disturbs
/// another.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn illegal_commands_are_refused_and_leave_the_session_alive() {
    /// Asserts a command is turned down, and turned down for the stated reason.
    ///
    /// The reason matters as much as the refusal. A command refused because the
    /// game misunderstood it looks identical to one refused correctly, and only
    /// the reason tells them apart. It is compared as a value and not as prose,
    /// which is the whole point of `Reason` being a type.
    fn refused(game: &mut Oracle<V>, command: impl Into<Step<V>>, want: Reason<V>) {
        let command = command.into();
        let refusal = game
            .step(&command)
            .expect("a refusal is still a reply, not a transport failure")
            .expect_err(&format!("{command:?} should have been refused"));
        assert_eq!(
            refusal.reason, want,
            "{command:?} was refused, but not for the stated reason"
        );
    }

    /// The same check for a command the typed API cannot build.
    ///
    /// Only for ids the content tables deliberately do not have: a name the
    /// game refuses, or a `RoomType` codegen drops. A command that ought to work
    /// and cannot be built is a gap in the tables.
    ///
    /// Prose here, and only here. Every one is a `Malformed` or a `Console`, the
    /// two reasons that carry a message.
    fn refused_raw(game: &mut Oracle<V>, command: serde_json::Value, expect: &str) {
        let refusal = game
            .send_raw(&command)
            .expect("a refusal is still a reply, not a transport failure")
            .expect_err(&format!("{command} should have been refused"));
        let message = match &refusal.reason {
            Reason::Malformed { message } | Reason::Console { message } => message,
            other => panic!("{command} should be malformed or declined, got {other:?}"),
        };
        assert!(
            message.to_lowercase().contains(&expect.to_lowercase()),
            "{command} was refused, but not for the stated reason:\n  \
             wanted something mentioning {expect:?}\n  got {message:?}"
        );
    }

    let mut game = launch();
    start(&mut game);

    // Out of combat, so nothing that needs one may proceed.
    refused(
        &mut game,
        Decision::PlayCard {
            index: hand(0),
            target: None,
        },
        Reason::NotInCombat,
    );
    refused(&mut game, Decision::EndTurn, Reason::NotInCombat);
    refused(
        &mut game,
        Decision::Buy { index: 0 },
        // A run opens in an event room, which the typed reason says and the
        // sentence it replaced could not.
        Reason::WrongRoom {
            wanted: RoomType::Shop,
            found: Some(RoomType::Event),
        },
    );

    // Nothing to answer.
    refused(
        &mut game,
        Decision::Answer(Answer::Skip),
        Reason::NothingPending,
    );

    // Names the tables do not have, so no `Command` can carry them. Reaching
    // these means going around the typed API, which is what `refused_raw` is
    // for, and the message comes from the game instead of from a guess here.
    refused_raw(
        &mut game,
        json!({"cmd": "grant", "relic": "NOT_A_REAL_RELIC"}),
        "NOT_A_REAL_RELIC",
    );

    // A grant naming neither. `Gift` cannot spell it, so this is the only thing
    // left that asks the mod whether it still refuses it.
    refused_raw(&mut game, json!({"cmd": "grant"}), "requires");

    // Setting up a run, where every argument is typed on this side and so every
    // one of these needs `send_raw` to reach. All three used to throw inside the
    // mod and arrive as refusals only because the catch-all flattened the two
    // together; now that a fault is its own message, a request that is merely
    // not a request has to be turned down as one.
    refused_raw(
        &mut game,
        json!({"cmd": "start", "seed": "ABCDEF", "character": "IRONCLAD", "ascension": "Ludicrous"}),
        "ascension must be",
    );
    refused_raw(
        &mut game,
        json!({"cmd": "start", "seed": "ABCDEF", "character": "IRONCLAD",
               "modifiers": ["NOT_A_REAL_MODIFIER"]}),
        "no such modifier",
    );
    refused_raw(
        &mut game,
        json!({"cmd": "grant", "card": "NOT_A_REAL_CARD"}),
        "NOT_A_REAL_CARD",
    );
    refused_raw(
        &mut game,
        json!({"cmd": "fight", "encounter": "NOT_A_REAL_ENCOUNTER"}),
        "not found",
    );

    // Legal shape, illegal move: the refusal lists what would have worked.
    refused(
        &mut game,
        Decision::Travel {
            to: Coord::new(99, 99),
        },
        Reason::NotTravelable {
            at: Coord::new(99, 99),
        },
    );

    // Eight refusals later the session still advances. `start` rather than
    // `observe`, which this client does not build — a command that changes
    // something is a better proof of life than one that only reports.
    let after = start(&mut game);
    assert!(after.state.is_object());
    assert!(after.errors.is_empty(), "a refusal should log nothing");
}

/// A healthy run reports no game errors at all.
///
/// Unpatched, the game reports one on every card played. The model layer calls
/// presentation inline, `CreatureCmd` asks for a screen shake when damage lands,
/// and headless has no screen to shake. `sts2_headless` patches that out, so a
/// non-empty `gameErrors` means something again. The field catches a
/// half-completed action, and cannot while it fires every turn.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn an_ordinary_card_play_logs_no_game_errors() {
    let mut game = launch();
    start(&mut game);

    let fought = constructed(
        &mut game,
        &Setup::Fight {
            encounter: ENCOUNTER,
        },
    );
    assert!(fought.errors.is_empty(), "entering a fight logged errors");

    // Index 0 is the Strike in this encounter's opening hand — which is
    // `["STRIKE_IRONCLAD", "BASH", "DEFEND_IRONCLAD", ...]` — and creature 1 is
    // the Bowlbug Rock. Both come from the observation above.

    let played = played(
        &mut game,
        &Decision::PlayCard {
            index: hand(0),
            target: Some(at(1)),
        },
    );
    assert!(played.errors.is_empty(), "playing a card logged errors");
    assert_eq!(played.decided, Some(Decided::Resolved));
}

/// `Unassigned` is a `RoomType` the enum carries and nothing can be jumped to.
/// It is the "no map point" placeholder `fight` lands on. Left in the enum it
/// parses, appears in the refusal message, and reaches `EnterRoomDebug`.
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

    let Reason::Malformed { message } = &refusal.reason else {
        panic!("a type the tables drop is not a request: {refusal:?}");
    };
    assert!(
        !message.contains("Unassigned"),
        "refused, but still offers it: {message}"
    );
    // The message earns its keep by naming what would have worked.
    assert!(
        message.contains("Shop"),
        "should list the types that do work: {message}"
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
/// The bug a code review found and nothing covered. `Driver.Run` called
/// `Environment.Exit(1)`, which does not work from Godot's main loop, so the
/// process logged "Exiting." and ran until something killed it.
///
/// Launched directly rather than through [`Game`], to withhold the `LD_PRELOAD`
/// the client always supplies. Without libgcc, Harmony cannot patch,
/// `sts2_determinism` stays inert, and the driver refuses.
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
    let err = Oracle::<V>::launch(Config::new("/definitely/not/the/game"))
        .map(|_| ())
        .expect_err("should refuse");
    assert!(matches!(err, Error::GameNotFound(_)), "got {err:?}");
}

/// Buying the shop's card removal must not deadlock the driver.
///
/// The fourth of the deadlock class the crate README describes, and the first
/// the fuzzer found. Every refusal `buy` can give is now decided in front of the
/// purchase instead of read off it. The second half asserts the run really did
/// lose a card.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn buying_card_removal_asks_which_card_rather_than_hanging() {
    let mut game = launch();
    let begun = start(&mut game);
    let deck = begun.snapshot.observation.deck.len();

    let shop = played(
        &mut game,
        &Decision::EnterRoom {
            room: RoomType::Shop,
        },
    );

    let Some(Room::Shop { entries }) = &shop.snapshot.observation.room else {
        panic!(
            "a shop should report its stock, got {:?}",
            shop.snapshot.observation.room
        );
    };
    let removal = entries
        .iter()
        .find(|e| e.stock == Stock::CardRemoval)
        .expect("a shop sells card removal");
    assert!(
        removal.affordable,
        "a fresh run has 99 gold and a removal costs 75; this test needs it affordable"
    );

    // The assertion is that this returns at all. A transport error here is the
    // deadlock; the command timeout is what turns the hang into a failure.
    let parked = played(
        &mut game,
        &Decision::Buy {
            index: removal.index,
        },
    );

    match parked
        .asking()
        .expect("card removal should park a question")
    {
        Question::SelectCards { min, options, .. } => {
            assert_eq!(*min, 1, "a removal takes exactly one card");
            assert!(!options.is_empty(), "the deck should be offered");
        }
        other => panic!("expected a card selection, got {other:?}"),
    }

    // Returning is not the same as working: answer it and check the run moved.
    let after = played(&mut game, &Decision::Answer(Answer::Cards(vec![0])));
    assert_eq!(
        after.snapshot.observation.deck.len(),
        deck - 1,
        "a removal takes exactly one card out of the deck"
    );
    assert!(
        after.snapshot.observation.gold < begun.snapshot.observation.gold,
        "and it is paid for"
    );
}

/// A rest site option that asks a question must not deadlock the driver.
///
/// The first of the deadlock class the crate README describes, and the one that
/// says why only a live test catches it. SMITH is the case under test because
/// HEAL resolves by itself.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn a_rest_site_option_that_asks_a_question_still_replies() {
    let mut game = launch();
    start(&mut game);

    let seen = played(
        &mut game,
        &Decision::EnterRoom {
            room: RoomType::RestSite,
        },
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
    let parked = played(&mut game, &Decision::ChooseOption { index: smith as u8 });

    match parked.asking().expect("SMITH should park a question") {
        Question::SelectCards { min, options, .. } => {
            assert_eq!(*min, 1, "smithing upgrades exactly one card");
            assert!(!options.is_empty(), "the deck should be offered");
        }
        other => panic!("expected a card selection, got {other:?}"),
    }

    // Returning is not the same as working: answer it and check the run moved.
    let after = played(&mut game, &Decision::Answer(Answer::Cards(vec![0])));

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

/// A recorded run replays into the same run, step for step.
///
/// The property everything else about a recording rests on. A recording stores
/// only the decisions, so it is meaningful only if the same decisions and seed
/// produce the same run, digest for digest.
///
/// A round trip in a unit test proves the encoding agrees with itself. This
/// proves it agrees with the game.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn a_recorded_run_replays_into_the_same_run() {
    let steps: Vec<Step<V>> = vec![
        Setup::Fight {
            encounter: ENCOUNTER,
        }
        .into(),
        Decision::PlayCard {
            index: hand(0),
            target: Some(at(1)),
        }
        .into(),
        Decision::EndTurn.into(),
    ];

    // Recorded by the middleware off the play path, not by pushing the same
    // list twice. What is on the tape has to be what the game was actually
    // told — a recording assembled from the caller's intentions would agree
    // with itself no matter what the wrapper did.
    let path = std::env::temp_dir().join(format!("sts2-replay-{}.jsonl", std::process::id()));
    let tape = recording::Writer::<V>::create(&path, &opening()).expect("a file to record into");
    let mut first = Recorder::new(launched(), tape);
    let mut digests = Vec::new();

    for step in &steps {
        let applied = first
            .step(step)
            .expect("the step should not fail at the transport level")
            .expect("the step should be accepted");
        digests.push(applied.observation.digest());
    }
    assert!(
        first.recording(),
        "the recording stopped part way: {:?}",
        first.fault()
    );
    drop(first);

    let file = std::io::BufReader::new(std::fs::File::open(&path).expect("the recording"));
    let read = recording::read::<V>(file).expect("what this build wrote, this build reads");
    assert_eq!(
        read.steps(),
        steps,
        "the tape is not the run that was played"
    );
    std::fs::remove_file(&path).ok();

    let mut second = launch();
    for (at, step) in read.steps().iter().enumerate() {
        assert_eq!(
            stepped(&mut second, step).digest(),
            digests[at],
            "step {} of the replay is a different state: {step:?}",
            at + 1
        );
    }
}

/// Every packed string in a real observation parses.
///
/// Held against what the game sends, rather than against a string this
/// repository wrote. The crate README says why that is the only check that can
/// catch the two disagreeing, and why every failure here names its path.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn every_packed_string_in_a_real_observation_parses() {
    let mut game = launch();
    let begun = start(&mut game);

    let fought = constructed(
        &mut game,
        &Setup::Fight {
            encounter: ENCOUNTER,
        },
    );

    let mut coords = 0;
    let mut cards = 0;
    let mut models = 0;

    for state in [&begun.state, &fought.state] {
        let mut at = |path: &str, text: &str| {
            coords += 1;
            sts2_headless::coord(text).unwrap_or_else(|e| panic!("{path} is {text:?}: {e}"));
        };

        let map = &state["map"];
        if let Some(text) = map["at"].as_str() {
            at("map.at", text);
        }
        for (i, c) in list(&map["visited"]).iter().enumerate() {
            at(&format!("map.visited[{i}]"), text_at(c, "map.visited"));
        }
        for (i, p) in list(&map["travelable"]).iter().enumerate() {
            at(&format!("map.travelable[{i}].at"), text_at(&p["at"], "at"));
        }
        for (i, p) in list(&map["points"]).iter().enumerate() {
            at(&format!("map.points[{i}].at"), text_at(&p["at"], "at"));
            for (j, to) in list(&p["to"]).iter().enumerate() {
                at(&format!("map.points[{i}].to[{j}]"), text_at(to, "to"));
            }
        }

        let mut ident = |path: &str, text: &str| {
            cards += 1;
            sts2_headless::card_ident::<V>(text)
                .unwrap_or_else(|e| panic!("{path} is {text:?}: {e}"));
        };

        let combat = &state["combat"];
        for pile in ["deck", "hand", "draw", "discard", "exhaust"] {
            let cards = match pile {
                "deck" => &state["deck"],
                other => &combat[other],
            };
            for (i, c) in list(cards).iter().enumerate() {
                ident(&format!("{pile}[{i}]"), text_at(c, pile));
            }
        }
        for (i, l) in list(&combat["legal"]).iter().enumerate() {
            ident(&format!("legal[{i}].card"), text_at(&l["card"], "card"));
        }

        let mut model = |path: &str, text: &str| {
            models += 1;
            sts2_headless::model_id::<V>(text)
                .unwrap_or_else(|e| panic!("{path} is {text:?}: {e}"));
        };

        for (i, c) in list(&combat["creatures"]).iter().enumerate() {
            // Null for a creature that is neither a monster nor a player, which
            // the mod guards for and nothing in this fight is.
            if let Some(text) = c["model"].as_str() {
                model(&format!("creatures[{i}].model"), text);
            }
        }
        for (i, l) in list(&combat["legal"]).iter().enumerate() {
            if let Some(text) = l["preventer"].as_str() {
                model(&format!("legal[{i}].preventer"), text);
            }
        }
    }

    // Counts, so that a run where every list came back empty cannot pass. The
    // map alone is dozens of points; the fight is a ten-card deck and two
    // monsters.
    assert!(coords > 20, "only {coords} coordinates — the map was empty");
    assert!(cards > 10, "only {cards} card identities");
    assert!(models >= 3, "only {models} model ids — no creatures?");
}

/// A JSON array, or nothing where the field is absent.
fn list(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

/// A JSON string, or a failure naming the field that was not one.
fn text_at<'a>(value: &'a Value, what: &str) -> &'a str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{what} should be a string, and is {value}"))
}

/// An enchantment reaches the deck and comes back in the card's identity.
///
/// The reason this exists. `Observation.Ident` used to write the id, the upgrade
/// level and the affliction, and stop. An enchantment is the third thing a card
/// carries into a pile. Two decks differing only by one digested the same, and a
/// simulator that got enchantments wrong passed.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn an_enchantment_travels_in_a_card_identity() {
    let mut game = launch();
    let begun = start(&mut game);

    let plain = deck(&begun.state);
    assert!(
        plain.iter().all(|c| !c.contains('*')),
        "a fresh deck carries no enchantment: {plain:?}"
    );

    let parked = constructed(&mut game, &Setup::Grant(Gift::Relic(Relic::RoyalStamp)));

    match parked.asking().expect("ROYAL_STAMP should park a question") {
        Question::SelectCards { min, options, .. } => {
            assert_eq!(*min, 1, "Royal Stamp enchants exactly one card");
            assert!(!options.is_empty(), "the deck should be offered");
        }
        other => panic!("expected a card selection, got {other:?}"),
    }

    let after = played(&mut game, &Decision::Answer(Answer::Cards(vec![0])));

    let enchanted: Vec<&str> = deck(&after.state)
        .into_iter()
        .filter(|c| c.contains('*'))
        .collect();
    assert_eq!(
        enchanted.len(),
        1,
        "exactly one card should be enchanted, deck was {:?}",
        deck(&after.state)
    );

    // The whole point: it parses, and into the enchantment the relic applies.
    let ident = sts2_headless::card_ident::<V>(enchanted[0])
        .unwrap_or_else(|e| panic!("{}: {e}", enchanted[0]));
    assert_eq!(ident.enchantment, Some(Enchantment::RoyallyApproved));
    assert_eq!(ident.affliction, None);
    assert_eq!(ident.upgrade, 0);

    // And the deck is one card longer in information than it was: the same
    // card without its enchantment is a different identity, which is what was
    // not true before the mod wrote this out.
    let bare = enchanted[0]
        .split('*')
        .next()
        .expect("an id before the star");
    assert_ne!(
        sts2_headless::card_ident::<V>(bare).unwrap(),
        ident,
        "the enchantment has to change the identity"
    );
}

/// The deck, as the observation spells it.
fn deck(state: &Value) -> Vec<&str> {
    list(&state["deck"])
        .iter()
        .map(|c| text_at(c, "deck"))
        .collect()
}

/// Every room the game can be standing in parses into an `Observation<V>`.
///
/// Held against the shapes the game writes, rather than against shapes this
/// repository wrote down. The rooms are walked deliberately rather than played
/// into. The crate README says why both matter.
#[test]
#[ignore = "launches the real game"]
#[serial(game)]
fn every_room_parses_into_an_observation() {
    let mut game = launch();

    let mut parsed = 0;
    let mut check = |what: &str, seen: &Seen| {
        let obs =
            sts2_headless::observation::<V>(&seen.state).unwrap_or_else(|e| panic!("{what}: {e}"));
        parsed += 1;
        obs
    };

    let begun = start(&mut game);
    let obs = check("start", &begun);
    assert!(!obs.deck.is_empty(), "a run starts with a deck");
    assert!(obs.combat.is_none(), "and not in a fight");
    assert!(!obs.map.points.is_empty(), "and on a map");
    assert_eq!(obs.rng.run.len(), 12, "twelve run streams");
    assert_eq!(obs.rng.player.len(), 3, "and three player ones");

    for room in [
        RoomType::RestSite,
        RoomType::Shop,
        RoomType::Treasure,
        RoomType::Event,
    ] {
        let seen = played(&mut game, &Decision::EnterRoom { room });
        let obs = check(&format!("{room:?}"), &seen);
        assert!(
            obs.room.is_some(),
            "{room:?} should report what it offers, got {:?}",
            obs.room
        );

        // The one room whose options are a closed set this build generates, so
        // the names really did come back as the enum rather than as anything a
        // string would have accepted. `Generate` makes these two and no relic
        // is held, so this is the whole list.
        if let Some(sts2_core::Room::RestSite { options }) = &obs.room {
            let ids: Vec<RestSiteOption> = options.iter().map(|o| o.id).collect();
            assert_eq!(
                ids,
                [RestSiteOption::Heal, RestSiteOption::Smith],
                "a rest site with no relics offers exactly these"
            );
        }
    }

    let fought = constructed(
        &mut game,
        &Setup::Fight {
            encounter: ENCOUNTER,
        },
    );
    let obs = check("fight", &fought);
    let combat = obs.combat.expect("a fight reports its combat");
    assert!(!combat.creatures.is_empty(), "a fight has creatures");
    assert!(!combat.legal.is_empty(), "and a hand to play from");
    assert!(
        combat.creatures.iter().any(|c| c.next_move.is_some()),
        "and at least one monster with a move chosen"
    );
    assert!(
        combat.legal.iter().any(|l| !l.effect.is_empty()),
        "and cards whose numbers were previewed"
    );

    let played = played(
        &mut game,
        &Decision::PlayCard {
            index: hand(0),
            target: Some(at(1)),
        },
    );
    check("after a card", &played);

    assert_eq!(parsed, 7, "every step should have been parsed");
}
