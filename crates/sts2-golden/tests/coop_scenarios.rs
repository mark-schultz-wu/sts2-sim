//! The simulator, against the game, on a co-op position nobody played to.
//!
//! A co-op scenario is a recording of several players. A `start` names
//! them, and each step after it names the seat whose step it is. A setup
//! puts the game into a position, and each game makes it for itself, so it
//! goes to every seat's machine. A decision is played by its seat. After
//! each step every seat's position is noted, in seat order.
//!
//! What the games answered lives in `coop-scenarios.bin`. Set `STS2_UPDATE`
//! to ask the games and write it, which needs v0.111.0 in `STS2_GAME_DIR`.
//! Without it this reads the file and needs no game.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, UdpSocket};
use std::path::{Path, PathBuf};

use sts2_content::v0_111_0::V0_111_0;
use sts2_core::{ContentVersion, Digested, RunTable, SeatIndex, Step as Taken};
use sts2_golden::scenario::{Expected, Seen, Stamp, Step};
use sts2_golden::suite::read_suite;
use sts2_headless::{Config, LobbyTable, SeatHost};
use sts2_sim::SimTable;

type V = V0_111_0;

fn updating() -> bool {
    std::env::var_os("STS2_UPDATE").is_some()
}

fn suite_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/coop-scenarios")
}

fn expected_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/coop-scenarios.bin")
}

/// Every step of `recorded` at `table`, and every seat's position after each.
fn walk<T: RunTable<V>>(
    table: &mut T,
    recorded: &sts2_core::Recording<V>,
) -> Result<Vec<Seen>, String>
where
    T::Error: std::fmt::Display,
{
    let seats = table.seats();
    let mut seen = Vec::new();
    for (at, (step, &seat)) in recorded.steps().iter().zip(recorded.seats()).enumerate() {
        let step_at = at + 1;
        match step {
            Taken::Decision(decision) => {
                table
                    .play(seat, decision)
                    .map_err(|e| format!("step {step_at}: {e}"))?
                    .map_err(|refusal| format!("step {step_at}: refused: {refusal}"))?;
            }
            // Each game puts itself into the position, so every machine
            // makes the setup.
            Taken::Setup(setup) => {
                for (machine, settled) in (table.construct_everywhere(setup))
                    .map_err(|e| format!("step {step_at}: {e}"))?
                    .into_iter()
                    .enumerate()
                {
                    settled.map_err(|refusal| {
                        format!("step {step_at}, seat {machine}: refused: {refusal}")
                    })?;
                }
            }
        }
        for machine in SeatIndex::all(seats) {
            let snapshot = table
                .snapshot(machine)
                .map_err(|e| format!("step {step_at}, seat {}: {e}", machine.0))?;
            seen.push(Seen {
                sections: snapshot.observation.sections().to_vec(),
                question: sts2_core::Digest::of_question_and_pile(snapshot.asking.as_ref()),
            });
        }
    }
    Ok(seen)
}

/// A lobby of `players` games, each with a profile of its own under `dir`.
fn lobby(dir: &Path, players: usize) -> Vec<SeatHost> {
    let game = sts2_headless::game_dir_from_env().expect("`STS2_GAME_DIR` names the v0.111.0 game");
    let port = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
        .and_then(|socket| socket.local_addr())
        .expect("a free port")
        .port();
    let dir = std::path::absolute(dir).expect("an absolute profile directory");
    (0..players)
        .map(|at| SeatHost {
            config: Config::new(&game).data_home(dir.join(format!("seat{at}"))),
            port,
        })
        .collect()
}

#[test]
fn the_simulator_reaches_the_co_op_positions_the_games_reach() {
    let written = read_suite(&suite_dir()).expect("the suite is readable");
    let stamp =
        serde_json::to_string(&sts2_headless::Stamp::of(&V::VERSION)).expect("a stamp is json");
    let held = match std::fs::read(expected_path()) {
        _ if updating() => Expected::default(),
        Ok(bytes) => Expected::decode(&bytes).expect("the answers parse"),
        Err(e) => panic!("no recorded answers: {e}\n    `STS2_UPDATE=1` records them"),
    };
    let mut fresh = Expected {
        stamp: Stamp {
            assembly: V::VERSION.assembly_hash,
            content: i64::from(V::VERSION.content_hash),
        },
        scenarios: BTreeMap::new(),
    };
    let mut wrong = Vec::new();

    for entry in &written {
        let name = &entry.name;
        let recording = format!("{stamp}\n{}", entry.commands);
        let recorded =
            sts2_headless::recording::read::<V>(std::io::Cursor::new(recording.as_bytes()))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        let players = recorded.start().players.len();

        if updating() {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/coop-scenarios")
                .join(name);
            let mut games = LobbyTable::<V>::begin(lobby(&dir, players), recorded.start())
                .unwrap_or_else(|e| panic!("{name}: starting the games: {e}"));
            let seen = walk(&mut games, &recorded).unwrap_or_else(|e| panic!("{name}: {e}"));
            games.finish(sts2_core::Verdict {
                end: sts2_core::End::Quit,
                step: recorded.steps().len(),
                act: 0,
                floor: 0,
                seat: None,
                place: None,
                detail: String::new(),
            });
            let mut steps = Vec::new();
            let mut before: Option<Seen> = None;
            for seen in seen {
                steps.push(Step::between(before.as_ref(), &seen));
                before = Some(seen);
            }
            fresh.scenarios.insert(name.clone(), steps);
            continue;
        }

        let steps = held
            .scenarios
            .get(name)
            .unwrap_or_else(|| panic!("{name} has no recorded answers; record with STS2_UPDATE"));
        let mut sim = SimTable::<V>::begin((), recorded.start())
            .unwrap_or_else(|e| panic!("{name}: starting: {e}"));
        let walked = walk(&mut sim, &recorded).unwrap_or_else(|e| panic!("{name}: {e}"));
        if let Some(why) = sts2_golden::scenario::parted(steps, &walked, players) {
            wrong.push(format!("{name}: {why}"));
        }
    }

    if updating() {
        std::fs::write(expected_path(), fresh.encode()).expect("the answers are writable");
        println!("recorded {} co-op scenarios", fresh.scenarios.len());
        return;
    }
    assert!(
        held.stamp
            == Stamp {
                assembly: V::VERSION.assembly_hash,
                content: i64::from(V::VERSION.content_hash),
            },
        "the answers came from another build; record with STS2_UPDATE"
    );
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}
