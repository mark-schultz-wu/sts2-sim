//! Constructing a scenario, instead of hunting for a seed that happens to be one.
//!
//! The shape is H6's: a scenario **names construction operations for the
//! oracle**, the oracle executes them and emits a full observation, and a
//! simulator starts from that observation. It never replays the construction —
//! `RelicCmd.Obtain` removes the relic from the grab bag and runs
//! `AfterObtained()`, which draws RNG for some relics, and reproducing all of
//! that would mean teaching the simulator a dev console it has no business
//! having.
//!
//! So a scenario is a sequence of commands here, not a file format. It becomes
//! a format when something needs it to be one.
//!
//! ```text
//! cargo run --example scenario
//! ```

use anyhow::bail;
use sts2_headless::{Command, Config, Oracle, Response};

/// Sends one command, prints what came back, and stops at the first refusal.
///
/// The two layers are the point: the outer `?` is "did the exchange happen",
/// the inner match is "did the oracle accept it". Without the second, a refused
/// command reads as success and every command after it runs against a state the
/// caller is not expecting.
fn step(oracle: &mut Oracle, command: Command) -> anyhow::Result<Response> {
    match oracle.send(&command)? {
        Ok(response) => {
            println!("{response}\n");
            Ok(response)
        }
        Err(refusal) => bail!("{command:?} was refused: {refusal}"),
    }
}

fn main() -> anyhow::Result<()> {
    let dir = sts2_headless::game_dir_from_env()
        .expect("set STS2_GAME_DIR — the repo's .envrc exports it, run `direnv allow`");
    let mut oracle = Oracle::launch(Config::new(dir))?;
    println!(
        "{}, protocol v{}\n",
        oracle.greeting().game,
        oracle.greeting().protocol
    );

    // 1. A fresh run, seeded explicitly. Nothing may start a run without one.
    step(
        &mut oracle,
        Command::Start {
            seed: Some("ORACLE1".into()),
            character: Some("IRONCLAD".into()),
            ascension: 0,
            modifiers: vec![],
        },
    )?;

    // 2. Edit it. Granting goes through the game's own `RelicCmd.Obtain` rather
    //    than writing to `Player.Relics`, so the effects that come with picking
    //    a relic up actually happen — watch the gold move, because Golden Pearl
    //    pays out on pickup.
    step(
        &mut oracle,
        Command::Grant {
            relic: Some("GOLDEN_PEARL".into()),
            card: None,
        },
    )?;

    // 3. A refusal is a value, not an exception. An encounter name the game does
    //    not know comes back with the game's own message: there is no id set on
    //    this side to validate against, so it answers better than we could.
    let wrong = Command::Fight {
        encounter: "NO_SUCH_ENCOUNTER".into(),
    };
    match oracle.send(&wrong)? {
        Ok(_) => bail!("expected the oracle to refuse an unknown encounter"),
        Err(refusal) => println!("refused, as it should be: {refusal}\n"),
    }

    // 4. The chosen fight. Reaching this one by travelling would mean searching
    //    for a seed that puts it where we want it, which is the seed-hunting
    //    this exists to avoid. The run lands off the map, so `map` and `combat`
    //    disagree — that is construction, not a reachable state.
    step(
        &mut oracle,
        Command::Fight {
            encounter: "BOWLBUGS_WEAK".into(),
        },
    )?;

    // 5. And it is an ordinary fight from here: play the Strike at the Rock.
    //    The hand indices and the target id are different numbering schemes —
    //    `index` is positional in hand, `target` is a CombatId that survives a
    //    creature dying.
    step(
        &mut oracle,
        Command::PlayCard {
            index: 2,
            target: Some(1),
        },
    )?;

    println!("The observation above is where a simulator would start.");
    Ok(())
}
