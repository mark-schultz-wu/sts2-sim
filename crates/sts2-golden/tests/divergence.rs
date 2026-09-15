//! How far the simulator gets through a real run before it disagrees.
//!
//! A recording is a seed and a sequence of decisions. The build is
//! deterministic, so driving those decisions into any correct implementation
//! must produce the same observations.
//!
//! Both are driven with the same steps and compared on
//! `Observation::rules_digest`. That covers the map, the legal moves and the
//! per-stream draw counts, and leaves out the strings a person reads.
//!
//! The oracle's half comes from the transcript `build.rs` filled, so this
//! needs no game. Holding the whole observation, rather than its hash alone,
//! is what lets a divergence name the fields that parted.
//!
//! `BASELINE` is how far the simulator got when this was last looked at. The
//! test fails if it gets less far, and says to raise it if it gets further.

use std::panic::{self, AssertUnwindSafe};

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{
    Combat, Construct, Creature, Decided, Play, Playable, Position, Positions, PowerAmount, Rules,
    Snapshot, Step,
};
use sts2_golden::{golden_dir, oracle_or_explain, recording};

type V = V0_107_1;

/// One run: the steps to replay, and how far they are known to get.
struct Golden<V: sts2_content_wire::WireContentVersion> {
    name: String,
    /// What the run was started with, which `Play::begin` takes and no step
    /// spells.
    start: sts2_core::Start<V>,
    steps: Vec<Step<V>>,
    /// How many steps the simulator is known to manage.
    baseline: usize,
}

/// Every run to replay, in a fixed order so a failure reads the same twice.
fn every_run<V: sts2_content_wire::WireContentVersion>() -> Vec<Golden<V>> {
    sts2_golden::every_run()
        .iter()
        .map(|name| golden(name))
        .collect()
}

fn golden<V: sts2_content_wire::WireContentVersion>(name: &str) -> Golden<V> {
    let path = golden_dir().join(name).join("BASELINE");
    let baseline = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let baseline = baseline
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("{name}: BASELINE is a step count: {e}"));

    let recorded = recording::<V>(name);
    Golden {
        name: name.to_string(),
        start: recorded.start().clone(),
        steps: recorded.steps().to_vec(),
        baseline,
    }
}

/// Why a replay stopped, in the words the report uses.
enum Stopped {
    /// The simulator cannot play the step at all.
    Fault(String),
    /// It played it and produced a different run.
    Diverged {
        want: String,
        got: String,
        /// Which fields parted. Never empty: something has to have differed.
        fields: Vec<String>,
    },
    /// It refused a decision the oracle accepted.
    Refused,
}

/// Replays until the first disagreement, and says how far that was.
///
/// Both halves are driven with the same steps. The oracle's answers come from
/// the transcript, so nothing here launches a game.
fn replay<V: Rules + sts2_content_wire::WireContentVersion>(
    golden: &Golden<V>,
) -> (usize, Option<Stopped>) {
    let mut oracle = oracle_or_explain(&golden.name, &golden.start);

    // Starting is not a step. `Play::begin` takes it, and is the one thing
    // with a single result: a run that did not start has nothing to report
    // from. A simulator that cannot even begin has got nowhere.
    let begun = panic::catch_unwind(AssertUnwindSafe(|| {
        sts2_sim::Sim::<V>::begin((), &golden.start)
    }));
    let mut sim = match begun {
        Ok(Ok(sim)) => sim,
        Ok(Err(e)) => return (0, Some(Stopped::Fault(e.to_string()))),
        Err(_) => return (0, Some(Stopped::Fault("it panicked starting".to_string()))),
    };

    for (at, step) in golden.steps.iter().enumerate() {
        let stepped = panic::catch_unwind(AssertUnwindSafe(|| sim.step(step)));
        if let Some(stopped) = compare(stepped, &mut sim, &mut oracle, step, at, golden) {
            return (at, Some(stopped));
        }
    }
    (golden.steps.len(), None)
}

/// What one step decided, and where it left the run, against the oracle's.
fn compare<V: Rules + sts2_content_wire::WireContentVersion, E: std::fmt::Display>(
    stepped: std::thread::Result<Result<sts2_core::Answered<V>, E>>,
    sim: &mut sts2_sim::Sim<V>,
    oracle: &mut impl Construct<V, Error = impl std::fmt::Display>,
    step: &Step<V>,
    at: usize,
    golden: &Golden<V>,
) -> Option<Stopped> {
    let wanted = oracle
        .step(step)
        .unwrap_or_else(|e| {
            panic!(
                "{}: the transcript covers its own recording: {e}",
                golden.name
            )
        })
        .unwrap_or_else(|refusal| panic!("{}: step {at} was refused: {refusal}", golden.name));
    // The oracle answers with both facts at once. Only what it decided is
    // wanted here: where it left the run is asked for below, of both halves
    // alike.
    let (wanted, _) = wanted.split();

    // A rule nobody has ported panics naming itself, rather than shape every
    // signature around not being written yet. That is a fault like any other,
    // and this is what keeps it one: the run still reports how far it got.
    let answered = match stepped {
        Ok(Ok(answered)) => answered,
        Ok(Err(e)) => return Some(Stopped::Fault(e.to_string())),
        Err(panicked) => return Some(Stopped::Fault(panic_message(&panicked))),
    };
    let Ok(made) = answered else {
        return Some(Stopped::Refused);
    };
    let (made, _) = made.split();

    // A decision says what it decided; where it left the run is the second
    // question, and comparing two implementations means asking both.
    let want = oracle
        .observe()
        .unwrap_or_else(|e| panic!("{}: step {at}: {e}", golden.name));
    let looked = panic::catch_unwind(AssertUnwindSafe(|| sim.observe()));
    let got = match looked {
        Ok(Ok(got)) => got,
        Ok(Err(e)) => return Some(Stopped::Fault(e.to_string())),
        Err(panicked) => return Some(Stopped::Fault(panic_message(&panicked))),
    };

    let diverged = made != wanted
        || digest_of(&got) != digest_of(&want)
        || words_apart(&want.observation.rng, &got.observation.rng);

    diverged.then(|| Stopped::Diverged {
        want: digest_of(&want),
        got: digest_of(&got),
        fields: differences(&wanted, &want, &made, &got),
    })
}

/// Whether two reports put a stream's generator in different places.
///
/// The digest holds the draw counts alone, because a build that saves the
/// count by itself reports no words. Where both sides carry the words they are
/// the sharper check: two streams stand at the same count and still hold
/// different numbers. The next draw off them says so, several moves late.
fn words_apart<V: sts2_content_wire::WireContentVersion>(
    want: &Positions<V>,
    got: &Positions<V>,
) -> bool {
    let run = want
        .run
        .iter()
        .any(|(stream, at)| got.run.get(stream).is_some_and(|mine| apart(*at, *mine)));
    let player = want
        .player
        .iter()
        .any(|(stream, at)| got.player.get(stream).is_some_and(|mine| apart(*at, *mine)));
    run || player
}

/// Two positions whose words are both known and disagree.
fn apart(want: Position, got: Position) -> bool {
    matches!((want.words, got.words), (Some(want), Some(got)) if want != got)
}

/// A stream's position, as a line a reader can compare.
fn standing(at: Option<&Position>) -> String {
    match at {
        None => "absent".to_string(),
        Some(Position {
            counter,
            words: None,
        }) => counter.to_string(),
        Some(Position {
            counter,
            words: Some([s0, s1, s2, s3]),
        }) => format!("{counter} [{s0:#018x} {s1:#018x} {s2:#018x} {s3:#018x}]"),
    }
}

/// What a panic said, which for an unported rule is what it named.
fn panic_message(panicked: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = panicked.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = panicked.downcast_ref::<&str>() {
        message.to_string()
    } else {
        "panicked".to_string()
    }
}

/// The digest of what the step produced, spelled the way the golden file is.
fn digest_of<V: sts2_content_wire::WireContentVersion>(applied: &Snapshot<V>) -> String {
    format!("{}", applied.observation.rules_digest())
}

#[test]
fn the_simulator_replays_real_runs() {
    let mut wrong = Vec::new();

    // An unported rule panics on purpose, and the report below says so in one
    // line. The default hook would print the same thing again with a stack.
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));

    for golden in every_run::<V>() {
        let total = golden.steps.len();
        let (reached, stopped) = replay(&golden);

        println!("\n{}: {reached}/{total} steps", golden.name);
        match &stopped {
            Some(Stopped::Fault(why)) => println!("  step {reached}: {why}"),
            Some(Stopped::Refused) => {
                println!("  step {reached}: refused, and the oracle accepted it")
            }
            Some(Stopped::Diverged { want, got, fields }) => {
                println!("  step {reached}: a different run");
                println!("    oracle {want}");
                println!("    sim    {got}");
                if fields.is_empty() {
                    println!("    in a field this report does not cover yet");
                }
                for field in fields {
                    println!("{field}");
                }
            }
            None => println!("  the whole run"),
        }

        // Collected rather than panicked on at the first, so a change that
        // moves two runs says so once.
        if reached < golden.baseline {
            wrong.push(format!(
                "{}: {reached}/{total}, down from {}. Something the simulator \
                 used to get right it no longer does.",
                golden.name, golden.baseline
            ));
        } else if reached > golden.baseline {
            wrong.push(format!(
                "{}: {reached}/{total}, up from {}. Raise \
                 tests/golden/{}/BASELINE to {reached} so the next change \
                 cannot lose it again.",
                golden.name, golden.baseline, golden.name
            ));
        }
    }

    panic::set_hook(hook);
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Which fields of an observation two implementations disagree on.
///
/// Every field, rather than the first: one wrong number usually moves several,
/// and the shortest list of them is what says which rule is missing.
fn differences<V: sts2_content_wire::WireContentVersion>(
    wanted: &Decided,
    want: &Snapshot<V>,
    made: &Decided,
    got: &Snapshot<V>,
) -> Vec<String> {
    let mut out = Vec::new();
    if wanted != made {
        out.push(format!(
            "    decided\n      oracle {wanted:?}\n      sim    {made:?}"
        ));
    }
    if want.asking != got.asking {
        out.push(format!(
            "    asking\n      oracle {:?}\n      sim    {:?}",
            want.asking, got.asking
        ));
    }
    let (want, got) = (&want.observation, &got.observation);
    let mut note = |field: &str, want: String, got: String| {
        if want != got {
            out.push(format!(
                "    {field}\n      oracle {want}\n      sim    {got}"
            ));
        }
    };

    note("floor", want.floor.to_string(), got.floor.to_string());
    note("act", want.act.to_string(), got.act.to_string());
    note("hp", want.hp.to_string(), got.hp.to_string());
    note("max_hp", want.max_hp.to_string(), got.max_hp.to_string());
    note("gold", want.gold.to_string(), got.gold.to_string());
    note("alive", want.alive.to_string(), got.alive.to_string());
    note(
        "run_over",
        want.run_over.to_string(),
        got.run_over.to_string(),
    );
    note(
        "in_combat",
        want.in_combat.to_string(),
        got.in_combat.to_string(),
    );
    note(
        "potion_slots",
        want.potion_slots.to_string(),
        got.potion_slots.to_string(),
    );
    note(
        "relics",
        ids(&want.relics, |c| format!("{:?}", c.relic)),
        ids(&got.relics, |c| format!("{:?}", c.relic)),
    );
    note(
        "potions",
        ids(&want.potions, |h| {
            format!("{:?} {:?} {:?}", h.potion, h.reason, h.targets)
        }),
        ids(&got.potions, |h| {
            format!("{:?} {:?} {:?}", h.potion, h.reason, h.targets)
        }),
    );
    note(
        "deck",
        ids(&want.deck, |c| format!("{c:?}")),
        ids(&got.deck, |c| format!("{c:?}")),
    );
    note(
        "map.at",
        format!("{:?}", want.map.at),
        format!("{:?}", got.map.at),
    );
    note(
        "map.travelable",
        ids(&want.map.travelable, |r| format!("{:?}", r.at)),
        ids(&got.map.travelable, |r| format!("{:?}", r.at)),
    );
    note(
        "map.visited",
        ids(&want.map.visited, |c| format!("{c:?}")),
        ids(&got.map.visited, |c| format!("{c:?}")),
    );
    note(
        "map.proceed",
        want.map.proceed.to_string(),
        got.map.proceed.to_string(),
    );
    note("room", room(&want.room), room(&got.room));
    let mut player_streams: Vec<_> = want.rng.player.keys().copied().collect();
    player_streams.sort_by_key(|stream| format!("{stream:?}"));
    for stream in player_streams {
        note(
            &format!("rng.player.{stream:?}"),
            standing(want.rng.player.get(&stream)),
            standing(got.rng.player.get(&stream)),
        );
    }

    // The counters say which stream drew a different number of times, which is
    // usually the first thing to go wrong and the hardest to see any other way.
    let mut streams: Vec<String> = want.rng.run.keys().map(|k| format!("{k:?}")).collect();
    streams.sort();
    for stream in streams {
        let at = |c: &Positions<V>| {
            standing(
                c.run
                    .iter()
                    .find(|(k, _)| format!("{k:?}") == stream)
                    .map(|(_, at)| at),
            )
        };
        note(&format!("rng.run.{stream}"), at(&want.rng), at(&got.rng));
    }

    match (&want.combat, &got.combat) {
        (Some(want), Some(got)) => combat_differences(want, got, &mut out),
        (want, got) => {
            let said = |c: &Option<Combat<V>>| if c.is_some() { "a fight" } else { "none" };
            if want.is_some() != got.is_some() {
                out.push(format!(
                    "    combat\n      oracle {}\n      sim    {}",
                    said(want),
                    said(got)
                ));
            }
        }
    }
    out
}

fn combat_differences<V: sts2_content_wire::WireContentVersion>(
    want: &Combat<V>,
    got: &Combat<V>,
    out: &mut Vec<String>,
) {
    let mut note = |field: &str, want: String, got: String| {
        if want != got {
            out.push(format!(
                "    {field}\n      oracle {want}\n      sim    {got}"
            ));
        }
    };
    note("combat.turn", want.turn.to_string(), got.turn.to_string());
    note(
        "combat.round",
        want.round.to_string(),
        got.round.to_string(),
    );
    note(
        "combat.phase",
        format!("{:?}", want.phase),
        format!("{:?}", got.phase),
    );
    note(
        "combat.energy",
        want.energy.to_string(),
        got.energy.to_string(),
    );
    note(
        "combat.stars",
        want.stars.to_string(),
        got.stars.to_string(),
    );
    note(
        "combat.encounter",
        format!("{:?}", want.encounter),
        format!("{:?}", got.encounter),
    );
    for (name, want, got) in [
        ("hand", want.hand.as_slice(), got.hand.as_slice()),
        ("draw", want.draw.as_slice(), got.draw.as_slice()),
        ("discard", want.discard.as_slice(), got.discard.as_slice()),
        ("exhaust", want.exhaust.as_slice(), got.exhaust.as_slice()),
    ] {
        note(
            &format!("combat.{name}"),
            ids(want, |c| format!("{c:?}")),
            ids(got, |c| format!("{c:?}")),
        );
    }

    // Per creature, so a fight of four does not report as one long line.
    for index in 0..want.creatures.len().max(got.creatures.len()) {
        let (a, b) = (want.creatures.get(index), got.creatures.get(index));
        match (a, b) {
            (Some(a), Some(b)) => {
                let at = |c: &Creature<V>| {
                    format!(
                        "{:?} hp {}/{} block {} alive {} powers {} next {:?}",
                        c.model,
                        c.hp,
                        c.max_hp,
                        c.block,
                        c.alive,
                        ids(&c.powers, |s| match s.amount {
                            PowerAmount::Counted(amount) => format!("{:?}={amount}", s.power),
                            PowerAmount::Endless => format!("{:?}=endless", s.power),
                        }),
                        c.next_move,
                    )
                };
                note(&format!("combat.creatures[{index}]"), at(a), at(b));
            }
            _ => note(
                &format!("combat.creatures[{index}]"),
                a.map_or("absent".into(), |c| format!("{:?}", c.model)),
                b.map_or("absent".into(), |c| format!("{:?}", c.model)),
            ),
        }
    }

    for index in 0..want.legal.len().max(got.legal.len()) {
        let at = |p: Option<&Playable<V>>| {
            p.map_or("absent".to_string(), |p| {
                format!(
                    "{:?} cost {:?}/{:?} target {:?} reason {:?} targets {:?} vars {:?}",
                    p.card, p.cost, p.star_cost, p.target, p.reason, p.targets, p.effect,
                )
            })
        };
        note(
            &format!("combat.legal[{index}]"),
            at(want.legal.get(index)),
            at(got.legal.get(index)),
        );
    }
}

/// A room, without the prose. The digest leaves the strings out, so a report
/// that put them back names a difference that is not one.
fn room<V: sts2_content_wire::WireContentVersion>(room: &Option<sts2_core::Room<V>>) -> String {
    use sts2_core::Room;
    match room {
        None => "none".to_string(),
        Some(Room::Event {
            event,
            finished,
            options,
            ..
        }) => format!(
            "event {event:?} finished {finished} {}",
            ids(options, |o| format!(
                "{}:{}{}{}{:?}",
                o.index,
                if o.locked { "locked " } else { "" },
                if o.proceed { "proceed " } else { "" },
                if o.chosen { "chosen " } else { "" },
                o.relic
            ))
        ),
        Some(Room::RestSite { options }) => format!(
            "rest {}",
            ids(options, |o| format!("{}:{:?}={}", o.index, o.id, o.enabled))
        ),
        Some(Room::Treasure { finished, options }) => format!(
            "treasure finished {finished} {}",
            ids(options, |o| format!("{}:{:?}", o.index, o.take))
        ),
        Some(Room::Shop { entries }) => format!(
            "shop {}",
            ids(entries, |e| format!(
                "{}:{:?} {}g stocked {} affordable {} {:?}",
                e.index, e.stock, e.cost, e.stocked, e.affordable, e.blocked
            ))
        ),
    }
}

/// A list, as the short line a report puts on one row.
fn ids<T>(items: &[T], one: impl Fn(&T) -> String) -> String {
    format!("[{}]", items.iter().map(one).collect::<Vec<_>>().join(", "))
}
