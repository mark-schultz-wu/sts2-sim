//! Constructing a scenario, instead of hunting for a seed that happens to be one.
//!
//! The shape is H6's: a scenario **names construction operations**, the game
//! executes them and emits a full observation, and a simulator starts from that
//! observation. It never replays the construction. `RelicCmd.Obtain` removes the
//! relic from the grab bag and runs `AfterObtained()`, which draws RNG for some
//! relics. Reproducing all of that means teaching the simulator a dev console it
//! has no business having.
//!
//! So a scenario is a sequence of commands here, not a file format. It becomes
//! a format when something needs it to be one.
//!
//! ```text
//! cargo run --example scenario
//! ```

use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_core::ContentVersion;
use sts2_core::Seed;
use sts2_core::{Applied, Construct, Decision, Gift, Play, Refusal, Setup, Start, Step};
use sts2_headless::{Config, Oracle};

type V = V0_107_1;
type AscensionLevel = <V as ContentVersion>::AscensionLevel;
type Character = <V as ContentVersion>::Character;
type Encounter = <V as ContentVersion>::Encounter;
type Relic = <V as ContentVersion>::Relic;

/// Construct a scenario against the game, printing each state it reaches.
#[derive(Parser)]
struct Cli {}

/// Why the scenario stopped.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("set STS2_GAME_DIR — the repo's .envrc exports it, run `direnv allow`")]
    NoGameDir,
    #[error(transparent)]
    Seed(#[from] sts2_core::seed::Error),
    #[error(transparent)]
    Game(#[from] sts2_headless::Error),
    #[error("{step:?} was refused: {refusal}")]
    Refused { step: Step<V>, refusal: Refusal<V> },
    #[error("expected the game to refuse an unknown encounter")]
    NotRefused,
}

/// Sends one command, prints what came back, and stops at the first refusal.
///
/// The two layers are the point: the outer `?` is "did the exchange happen",
/// the inner match is "did the game accept it". Without the second, a refused
/// command reads as success and every command after it runs against a state the
/// caller is not expecting.
fn step(game: &mut Oracle<V>, command: impl Into<Step<V>>) -> Result<Applied<V>, Error> {
    let step = command.into();
    match game.step(&step)? {
        Ok(applied) => {
            // Printed from the reply the oracle kept, because the rule line at
            // the top of each step is the protocol's envelope and `Applied` does
            // not carry one.
            let seen = applied.clone().snapshot();
            let view = sts2_cli::Readable::new(&seen).logging(game.logged());
            println!("{view}\n");
            Ok(applied)
        }
        Err(refusal) => Err(Error::Refused { step, refusal }),
    }
}

fn main() -> ExitCode {
    let Cli {} = Cli::parse();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Error> {
    let dir = sts2_headless::game_dir_from_env().ok_or(Error::NoGameDir)?;
    // 1. A fresh run, seeded explicitly. Starting one is how a game is
    //    reached at all: nothing plays without a run behind it.
    let mut game = Oracle::begin(
        Config::new(dir),
        &Start {
            seed: Seed::new("0RACLE1")?,
            character: Character::Ironclad,
            ascension: AscensionLevel::None,
            modifiers: vec![],
        },
    )?;
    println!(
        "{}, mod {}\n",
        game.greeting().game,
        game.greeting()
            .identity
            .as_deref()
            .unwrap_or("unidentified")
    );

    // 2. Edit it. Granting goes through the game's own `RelicCmd.Obtain` rather
    //    than writing to `Player.Relics`, so the effects that come with picking
    //    a relic up actually happen — watch the gold move, because Golden Pearl
    //    pays out on pickup.
    step(&mut game, Setup::Grant(Gift::Relic(Relic::GoldenPearl)))?;

    // 3. A refusal is a value, not an exception. An encounter name the game
    //    does not know cannot be built as a `Command` at all now — the tables
    //    have every real one — so asking for a refusal means going around the
    //    typed API deliberately, which is what `send_raw` is for.
    let wrong = serde_json::json!({"cmd": "fight", "encounter": "NO_SUCH_ENCOUNTER"});
    match game.send_raw(&wrong)? {
        Ok(_) => return Err(Error::NotRefused),
        Err(refusal) => println!("refused, as it should be: {refusal}\n"),
    }

    // 4. The chosen fight. Reaching this one by travelling would mean searching
    //    for a seed that puts it where we want it, which is the seed-hunting
    //    this exists to avoid. The run lands off the map, so `map` and `combat`
    //    disagree — that is construction, not a reachable state.
    step(
        &mut game,
        Setup::Fight {
            encounter: Encounter::BowlbugsWeak,
        },
    )?;

    // 5. And it is an ordinary fight from here: play the Strike at the Rock.
    //    The hand indices and the target id are different numbering schemes —
    //    `index` is positional in hand, `target` is a CombatId that survives a
    //    creature dying.
    step(
        &mut game,
        Decision::PlayCard {
            index: sts2_core::fixture::hand(2),
            target: Some(sts2_core::fixture::at(1)),
        },
    )?;

    println!("The observation above is where a simulator would start.");
    Ok(())
}
