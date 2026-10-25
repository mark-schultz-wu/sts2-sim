//! Run setup, against what the game drew for the same seed.
//!
//! `tests/setup.txt` is a real run's `UpFront` stream, phase by phase, taken by
//! `cargo xtask dump setupdump`. Each `phase` line is where the counter stood
//! afterwards, and each `chose` line is what that phase picked.
//!
//! The counter and the picks are both checked. A bag shuffled into a different
//! order lands on the same count, so the count alone says very little.
//!
//! What is ported so far is the two relic bags. The rest of the file is the map
//! of what is not.

use std::collections::HashMap;
use std::path::Path;

use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::characters::Character;
use sts2_core::rng::{GameRng, stream_seed};
use sts2_core::{ContentVersion, Rules, Seed, Table};
use sts2_sim::rng::MegaRng;
use sts2_sim::run::{Opening, RelicBag};

type V = V0_107_1;

/// The seed the fixture was taken on.
const SEED: &str = "0RACLE1";

/// The fixture, as its two kinds of line.
///
/// `phases` is name to counter, `chose` is name to the ids it picked. Both keep
/// the file's own names, so a failure quotes something greppable.
struct Fixture {
    phases: Vec<(String, i32)>,
    chose: HashMap<String, Vec<String>>,
    /// The pools each phase read, before anything shuffled them.
    order: HashMap<String, Vec<String>>,
}

fn fixture() -> Fixture {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/setup.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));

    let mut phases = Vec::new();
    let mut chose = HashMap::new();
    let mut order = HashMap::new();
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let mut word = line.split_whitespace();
        match word.next() {
            Some("phase") => {
                let name = word.next().expect("a phase is named").to_string();
                let at = word
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or_else(|| panic!("{line:?} has no counter"));
                phases.push((name, at));
            }
            Some("chose") => {
                let name = word.next().expect("a choice is named").to_string();
                chose.insert(name, word.map(ToString::to_string).collect());
            }
            Some("order") => {
                let name = word.next().expect("a pool is named").to_string();
                order.insert(name, word.map(ToString::to_string).collect());
            }
            _ => {}
        }
    }
    assert!(!phases.is_empty(), "{} holds no phases", path.display());
    Fixture {
        phases,
        chose,
        order,
    }
}

/// Where the counter stood after a named phase.
fn at(fixture: &Fixture, phase: &str) -> i32 {
    fixture
        .phases
        .iter()
        .find(|(name, _)| name == phase)
        .unwrap_or_else(|| panic!("the fixture has no phase {phase:?}"))
        .1
}

/// A bag's deques as the fixture spells them: `<phase>.<rarity>` to ids.
fn spell(bag: &RelicBag<V>) -> Vec<(String, Vec<String>)> {
    bag.deques()
        .map(|(rarity, relics)| {
            let ids: Vec<String> = relics.iter().map(id).collect();
            (format!("{rarity:?}"), ids)
        })
        .collect()
}

/// The whole of what the run drew before the player did anything.
///
/// One stream, in the game's own order, because the order is half of what is
/// being checked. Where this stops is where the port stops.
#[test]
fn the_relic_bags_are_shuffled_the_way_the_game_shuffled_them() {
    let fixture = fixture();
    let seed = Seed::new(SEED).expect("the fixture's seed is a seed");
    let mut rng = sts2_core::rng::named::<V, MegaRng>(stream_seed::<V>(&seed), "up_front");

    assert_eq!(rng.counter(), at(&fixture, "created"), "before anything");

    let shared = RelicBag::<V>::shared(&mut rng);
    assert_eq!(
        rng.counter(),
        at(&fixture, "sharedrelics"),
        "the shared bag drew the wrong number of times"
    );
    check(&fixture, "sharedrelics", &shared);

    let player = RelicBag::<V>::player(&mut rng, Character::Regent);
    assert_eq!(
        rng.counter(),
        at(&fixture, "playerrelics"),
        "the player's bag drew the wrong number of times"
    );
    check(&fixture, "playerrelics", &player);
}

/// The whole of a run's setup, phase by phase, against what the game drew.
///
/// The counter is checked at each phase boundary and every choice against the
/// fixture. Reaching 413 with the wrong rooms is a failure this catches, and
/// reaching the right rooms at the wrong counter is one too.
#[test]
fn setting_up_a_run_draws_what_the_game_drew() {
    let fixture = fixture();
    let seed = Seed::new(SEED).expect("the fixture's seed is a seed");
    let mut rng = sts2_core::rng::named::<V, MegaRng>(stream_seed::<V>(&seed), "up_front");

    let acts = sts2_sim::run::run_acts::<V>(stream_seed::<V>(&seed));
    let opening = Opening::<V>::draw(
        &mut rng,
        &[Character::Regent],
        &acts,
        sts2_content::v0_107_1::ascension::AscensionLevel::None,
    );

    let mut report = String::new();
    for (act, rooms) in &opening.acts {
        let name = id(act);
        compare(
            &fixture,
            &mut report,
            &format!("normal.{name}"),
            &ids(&rooms.normal),
        );
        compare(
            &fixture,
            &mut report,
            &format!("elite.{name}"),
            &ids(&rooms.elite),
        );
        compare(
            &fixture,
            &mut report,
            &format!("events.{name}"),
            &ids(&rooms.events),
        );
        compare(
            &fixture,
            &mut report,
            &format!("boss.{name}"),
            &[id(rooms.boss)],
        );
        compare(
            &fixture,
            &mut report,
            &format!("ancient.{name}"),
            &[id(rooms.ancient)],
        );
    }
    assert!(report.is_empty(), "the rooms differ\n{report}");

    assert_eq!(
        rng.counter(),
        at(
            &fixture,
            &format!("rooms.{}", id(acts.last().expect("a run has acts")))
        ),
        "setup drew the wrong number of times overall"
    );
}

/// Ids of a list, as the fixture spells them.
fn ids<T: serde::Serialize>(items: &[T]) -> Vec<String> {
    items.iter().map(id).collect()
}

/// One `chose` line against what was generated, appending to a report.
fn compare(fixture: &Fixture, report: &mut String, key: &str, got: &[String]) {
    let Some(want) = fixture.chose.get(key) else {
        report.push_str(&format!("  {key}: the fixture has no such line\n"));
        return;
    };
    if got == want.as_slice() {
        return;
    }
    let first = got.iter().zip(want).position(|(a, b)| a != b);
    report.push_str(&format!(
        "  {key}: {} against {}, first differing at {first:?}\n",
        got.len(),
        want.len()
    ));
    if let Some(at) = first {
        report.push_str(&format!("    got {:?} want {:?}\n", got[at], want[at]));
    }
}

/// Every deque against the fixture, by rarity, reporting all of them.
fn check(fixture: &Fixture, phase: &str, bag: &RelicBag<V>) {
    let mut report = String::new();
    for (rarity, ids) in spell(bag) {
        let key = format!("{phase}.{rarity}");
        let Some(want) = fixture.chose.get(&key) else {
            report.push_str(&format!("  {key}: the fixture has no such deque\n"));
            continue;
        };
        if &ids != want {
            let first = ids
                .iter()
                .zip(want)
                .position(|(mine, theirs)| mine != theirs);
            report.push_str(&format!(
                "  {key}: {} relics against {}, first differing at {first:?}\n",
                ids.len(),
                want.len()
            ));
        }
    }
    let mine: Vec<String> = spell(bag).into_iter().map(|(r, _)| r).collect();
    let theirs: Vec<String> = fixture
        .chose
        .keys()
        .filter_map(|key| key.strip_prefix(&format!("{phase}.")))
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        mine.len(),
        theirs.len(),
        "{phase}: {} deques against the fixture's {}",
        mine.len(),
        theirs.len()
    );
    assert!(report.is_empty(), "{phase} differs\n{report}");
}

/// An act's encounters, in the order the content table lists them.
///
/// The game's own order, which the dump used to sort away. A grab bag is
/// filled by walking this, so the order decides what a draw picks.
#[test]
fn every_act_fields_its_encounters_in_the_game_s_own_order() {
    let fixture = fixture();
    let mut checked = 0;
    for act in <V as ContentVersion>::Act::ALL {
        let key = format!("encounters.{}", id(act));
        let Some(want) = fixture.order.get(&key) else {
            continue;
        };
        let got: Vec<String> = V::encounters(*act).iter().map(id).collect();
        assert_eq!(&got, want, "{key} is in the wrong order");
        checked += 1;
    }
    assert!(checked >= 3, "the fixture names {checked} acts, not three");
}

/// Anything with a serde rename, spelled the way the game spells it.
fn id<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .expect("a content id serializes")
        .as_str()
        .expect("a content id is a string")
        .to_string()
}
