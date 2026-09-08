//! The encounter table against the dump it came from.
//!
//! Structured like the other dumps' tests: replay the dump, row by row, against
//! the table it generated. Two things here are not like the others.
//!
//! The first is `roster`. Whether `GenerateMonsters` is a table at all was
//! decided by running it and reading the `Rng` counter afterwards. So the set of
//! encounters that roll is a claim about the build, not something visible in the
//! generated file. It is pinned by name.
//!
//! The second is that this dump and the monster dump both project the same
//! relation. The two halves are dumped separately, they have to agree, and
//! nothing but a test says so.

use super::super::encounters::*;
use super::super::monsters::Monster;
use super::super::{ENCOUNTERS, MONSTERS, VERSION};
use super::{build, rows, wire};
use std::collections::{BTreeMap, BTreeSet};
use sts2_core::{EncounterRoster, EncounterTable, MonsterTable, Table};

fn encounter(id: &str) -> Encounter {
    *Encounter::ALL
        .iter()
        .find(|e| wire(e) == id)
        .unwrap_or_else(|| panic!("{id} is in the dump but not generated"))
}

/// The members `DROPPED` in `xtask/src/codegen/encounters.rs`, spelled out again
/// so that dropping another one has to be said twice.
const DROPPED: [(&str, &str); 2] = [("Family", "None"), ("RoomType", "Unassigned")];

/// The anchor: a dump from another build has another hash, and a table
/// regenerated against one is the failure this prevents.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(ENCOUNTERS),
        VERSION,
        "encounters.txt was dumped from a different build than this module claims"
    );
}

#[test]
fn every_encounter_is_generated() {
    let dumped: Vec<&str> = rows(ENCOUNTERS, "encounter").map(|r| r[1]).collect();
    assert_eq!(dumped.len(), 96, "the build has 96 encounters");
    assert_eq!(Encounter::ALL.len(), dumped.len());

    let mut seen = BTreeSet::new();
    for id in dumped {
        assert!(seen.insert(id), "duplicate encounter id {id}");
        assert_eq!(wire(&encounter(id)), id);
    }
}

/// Every column and every row kind, against the table it generated.
#[test]
fn the_generated_table_matches_the_dump() {
    let mut checked = 0;

    for row in rows(ENCOUNTERS, "encounter") {
        let id = row[1];
        let d = encounter(id).data();

        assert_eq!(format!("{:?}", d.room), row[2], "{id}: room");

        for (column, flag) in [
            (3, EncounterFlag::Weak),
            (4, EncounterFlag::GivesRewards),
            (5, EncounterFlag::Debug),
        ] {
            let set = if d.flags.contains(flag) { "1" } else { "0" };
            assert_eq!(set, row[column], "{id}: {flag:?}");
        }

        assert_eq!(d.gold.min.base.to_string(), row[6], "{id}: min gold");
        assert_eq!(d.gold.max.base.to_string(), row[7], "{id}: max gold");

        let roster = match d.roster {
            Roster::Fixed(_) => "fixed",
            Roster::Random => "random",
            Roster::Configured => "configured",
        };
        assert_eq!(roster, row[8], "{id}: roster");

        checked += 1;
    }

    assert_eq!(checked, 96);

    // The rows that hang off an encounter.
    // Which monsters, in what order. Where each one stands is `check_slots`,
    // because the table keeps a position where the dump keeps a name.
    check_rows(
        "spawn",
        true,
        |r| r[3].to_string(),
        |d| match d.roster {
            Roster::Fixed(spawns) => spawns.iter().map(|s| wire(&s.monster)).collect(),
            _ => Vec::new(),
        },
    );

    // Tags and possible monsters are sets, so they are compared as sets. The
    // game declares `Tags` as a sequence, but its one reader is
    // `Tags.Intersect(other.Tags).Any()` — which cannot see an order — so a
    // bitset is faithful even though it yields in member order rather than
    // declaration order. `FLYCONID_NORMAL` is where the two differ.
    check_rows(
        "tag",
        false,
        |r| r[2].to_string(),
        |d| d.families.iter().map(|f| wire(&f)).collect(),
    );
    check_rows(
        "possible",
        false,
        |r| r[2].to_string(),
        |d| d.possible.iter().map(|m| wire(&m)).collect(),
    );

    check_slots();
}

/// Slot names, which the table keeps only as counts and indices.
///
/// Rebuilt from the dump the way codegen builds it: declared slots first, then
/// any a spawn names and the encounter does not. So this is the rule itself
/// under test, and not codegen agreeing with codegen.
fn check_slots() {
    let mut declared: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for row in rows(ENCOUNTERS, "slot") {
        declared.entry(row[1]).or_default().push(row[3]);
    }

    let mut spawned: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for row in rows(ENCOUNTERS, "spawn") {
        spawned.entry(row[1]).or_default().push(row[4]);
    }

    for e in Encounter::ALL {
        let id = wire(&e);
        let d = e.data();
        let slots = declared.get(id.as_str()).cloned().unwrap_or_default();

        assert_eq!(d.arrivals as usize, slots.len(), "{id}: arrival slot count");

        let mut vocabulary = slots;
        for name in spawned.get(id.as_str()).cloned().unwrap_or_default() {
            if name != "-" && !vocabulary.contains(&name) {
                vocabulary.push(name);
            }
        }

        let Roster::Fixed(spawns) = d.roster else {
            continue;
        };
        let names = spawned.get(id.as_str()).cloned().unwrap_or_default();
        for (spawn, name) in spawns.iter().zip(names) {
            match spawn.slot {
                None => assert_eq!(
                    name, "-",
                    "{id}: a spawn has no slot but the dump named one"
                ),
                Some(slot) => assert_eq!(
                    vocabulary[slot as usize], name,
                    "{id}: {slot:?} is not where the dump put this monster"
                ),
            }
        }
    }
}

/// One kind of hanging row, gathered per encounter and compared as a list.
///
/// `ordered` says whether the comparison is a sequence or a set. A spawn's
/// index is which monster it is, so that one is ordered. Families and possible
/// monsters are sets in the table too. The bitset yields in member order rather
/// than the game's, which is faithful because the only reader is an
/// intersection.
fn check_rows(
    kind: &'static str,
    ordered: bool,
    value: impl Fn(&[&'static str]) -> String,
    read: impl Fn(&EncounterData) -> Vec<String>,
) {
    let mut dumped: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for row in rows(ENCOUNTERS, kind) {
        let v = value(&row);
        dumped.entry(row[1]).or_default().push(v);
    }

    for e in Encounter::ALL {
        let id = wire(&e);
        let mut got = read(e.data());
        let mut want = dumped.get(id.as_str()).cloned().unwrap_or_default();
        if !ordered {
            got.sort();
            want.sort();
        }
        assert_eq!(got, want, "{id}: {kind} rows");
    }
}

/// The two dumps project the same relation and have to agree on it.
///
/// `monsters.txt` says which monsters an act can field, and `encounters.txt`
/// says which encounters an act can roll. The game derives the first from the
/// second, since `ActModel.AllMonsters` is
/// `AllEncounters.SelectMany(e => e.AllPossibleMonsters).Distinct()`.
///
/// A disagreement means one dump is reading something the other is not, which
/// neither dump's own tests notice.
#[test]
fn the_two_dumps_agree_on_which_monsters_an_act_can_field() {
    // Keyed by the act's own spelling, which is also how `monsters.txt`
    // names it — `Act` is not `Ord`, and giving it an ordering for a test
    // would be inventing one the game does not have.
    let mut derived: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for e in Encounter::ALL {
        for act in e.acts() {
            derived
                .entry(format!("{act:?}").to_uppercase())
                .or_default()
                .extend(e.data().possible.iter().map(|m| wire(&m)));
        }
    }

    let mut dumped: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for row in rows(MONSTERS, "act") {
        dumped.entry(row[1]).or_default().insert(row[2].to_string());
    }

    assert_eq!(derived.len(), 4, "every act should roll some encounter");
    assert_eq!(derived.len(), dumped.len());

    for (name, monsters) in &derived {
        assert_eq!(
            monsters,
            dumped
                .get(name.as_str())
                .unwrap_or_else(|| panic!("{name} is in no monster dump row")),
            "{name}: the two dumps disagree on which monsters it can field"
        );
    }

    // Said from the monster table's side too, so the check is not just that
    // two projections of the encounter dump agree with each other.
    for m in Monster::ALL {
        for act in m.acts() {
            let name = format!("{act:?}").to_uppercase();
            assert!(
                derived[&name].contains(&wire(&m)),
                "{}: acts() names {act:?}, but no encounter there fields it",
                wire(&m)
            );
        }
    }
}

/// Exactly the encounters that do not have a roster, and no others.
///
/// This is the assertion the whole `Roster` split rests on, and reading the
/// generated file cannot check it. Whether an encounter rolls was decided by
/// running `GenerateMonsters` against a fresh `Rng` and reading the counter
/// afterwards. So the answer is pinned by name, and a build that makes a
/// fifteenth encounter roll has to say so here.
#[test]
fn only_the_known_encounters_lack_a_roster() {
    let with = |want: Roster| -> Vec<String> {
        Encounter::ALL
            .iter()
            .filter(|e| std::mem::discriminant(&e.data().roster) == std::mem::discriminant(&want))
            .map(wire)
            .collect()
    };

    assert_eq!(
        with(Roster::Random),
        [
            "BOWLBUGS_NORMAL",
            "BOWLBUGS_WEAK",
            "CORPSE_SLUGS_NORMAL",
            "CORPSE_SLUGS_WEAK",
            "DECIMILLIPEDE_ELITE",
            "FLYCONID_NORMAL",
            "PUNCH_OFF_EVENT_ENCOUNTER",
            "RUBY_RAIDERS_NORMAL",
            "SCROLLS_OF_BITING_NORMAL",
            "SCROLLS_OF_BITING_WEAK",
            "SLIMES_NORMAL",
            "SLIMES_WEAK",
            "SLITHERING_STRANGLER_NORMAL",
            "TWO_TAILED_RATS_NORMAL",
        ],
        "the set of encounters that roll their roster has changed"
    );

    // The Battleworn Dummy event picks which of three friends turns up, and the
    // encounter throws rather than generate without it.
    assert_eq!(
        with(Roster::Configured),
        ["BATTLEWORN_DUMMY_EVENT_ENCOUNTER"],
        "the set of encounters configured from outside has changed"
    );
}

/// A spawn's slot is not an index into `slots`, and three encounters prove it.
///
/// Worth pinning because the obvious modelling is wrong here, on three rows out
/// of 115. That modelling resolves the name into a position, the way a monster's
/// state ids become indices. `Slots` is what `GetNextSlot` hands to a creature
/// added mid-combat, and where a monster *starts* is whatever
/// `GenerateMonsters` names.
#[test]
fn a_spawn_can_start_in_a_slot_the_encounter_never_declares() {
    let mut undeclared: Vec<String> = Vec::new();

    for e in Encounter::ALL {
        let d = e.data();
        let Roster::Fixed(spawns) = d.roster else {
            continue;
        };

        // Past `arrivals` is exactly "a position the encounter never declared":
        // the index space is the declared slots first, then the rest.
        if spawns
            .iter()
            .filter_map(|s| s.slot)
            .any(|slot| slot as u8 >= d.arrivals)
        {
            undeclared.push(wire(&e));
        }
    }

    assert_eq!(
        undeclared,
        [
            "GREMLIN_MERC_NORMAL",
            "KNIGHTS_ELITE",
            "SLUMBERING_BEETLE_NORMAL"
        ],
        "the encounters starting a monster in an undeclared slot have changed"
    );
}

/// Gold moves at Poverty and nowhere else, except where an override ignores it.
///
/// The sweep reads both properties at all eleven ascension levels rather than
/// at the one the source names, and this is what that bought. Every encounter
/// steps at level 3. The one that does not step at all is the one that
/// overrides the properties outright.
#[test]
fn gold_steps_only_at_poverty() {
    let mut flat: Vec<String> = Vec::new();

    for e in Encounter::ALL {
        let gold = e.data().gold;
        for (which, scaled) in [("min", gold.min), ("max", gold.max)] {
            match scaled.step {
                Some((level, _)) => {
                    assert_eq!(level, 3, "{}: {which} gold steps at {level}", wire(&e))
                }
                None if which == "min" => flat.push(wire(&e)),
                None => {}
            }
        }
    }

    assert_eq!(
        flat,
        ["FAKE_MERCHANT_EVENT_ENCOUNTER"],
        "the encounters whose gold ignores Poverty have changed"
    );
}

/// The generated enums are the game's, minus exactly the dropped members, at
/// the game's own numbers.
///
/// Both members dropped here are a C# zero that means "no answer". Nothing
/// names `EncounterTag.None`, and every one of `RoomType.Unassigned`'s eight
/// uses is an absence.
///
/// Dropping leaves a hole rather than renumbering, so `Monster` stays 1
/// whatever else changes.
#[test]
fn the_generated_enums_drop_only_what_was_declared_dropped() {
    macro_rules! check_enum {
        ($ty:ty, $game:literal, $name:literal, $members:literal) => {
            let declared: Vec<_> = rows(ENCOUNTERS, "enum").filter(|r| r[1] == $game).collect();
            assert_eq!(declared.len(), $members, "{} member count changed", $name);

            for row in declared {
                let (member, discriminant) = (row[3], row[2].parse::<u8>().unwrap());
                let parsed: Option<$ty> = serde_json::from_str(&format!("\"{member}\"")).ok();

                match parsed {
                    Some(v) => {
                        assert!(
                            !DROPPED.contains(&($name, member)),
                            "{}::{member} is dropped but still parses",
                            $name
                        );
                        assert_eq!(
                            v as u8, discriminant,
                            "{}::{member} is not at the game's own number",
                            $name
                        );
                    }
                    None => assert!(
                        DROPPED.contains(&($name, member)),
                        "{}::{member} is declared by the game but does not parse",
                        $name
                    ),
                }
            }
        };
    }

    check_enum!(Family, "EncounterTag", "Family", 15);
    check_enum!(RoomType, "RoomType", "RoomType", 9);
}

/// Dropping `RoomType::Unassigned` is only safe because nothing here has it.
///
/// The other seven members stay, used or not. `Treasure`, `Shop`, `RestSite`
/// and `Map` are rooms an encounter is never in, and they are still rooms. What
/// makes `Unassigned` different is not that it is unused but that it means
/// nothing.
#[test]
fn every_encounter_is_in_a_room_that_holds_a_fight() {
    for row in rows(ENCOUNTERS, "encounter") {
        assert!(
            ["Monster", "Elite", "Boss"].contains(&row[2]),
            "{} is in a {} room",
            row[1],
            row[2]
        );
    }
}

/// The wire form round-trips, and it is the only spelling of an id.
///
/// There is no `id` accessor, because the serde rename is the id. So this is
/// the test saying the 96 renames are the 96 ids the dump declares, and that
/// every one of them reads back.
#[test]
fn an_encounter_round_trips_through_its_wire_form() {
    let dumped: Vec<&str> = rows(ENCOUNTERS, "encounter").map(|r| r[1]).collect();

    for (e, id) in Encounter::ALL.iter().zip(&dumped) {
        let json = serde_json::to_string(e).unwrap();
        assert_eq!(json, format!("\"{id}\""), "the rename is not the dump's id");
        assert_eq!(&serde_json::from_str::<Encounter>(&json).unwrap(), e);
    }

    assert!(serde_json::from_str::<Encounter>("\"NOT_AN_ENCOUNTER\"").is_err());
    assert!(serde_json::from_str::<Encounter>("\"KNIGHTS_ELIT\"").is_err());
}

/// Families exist for one reason, so the thing that reason needs is worth
/// having.
///
/// `ActModel` will not put an encounter next to one it shares a family with,
/// which is why two slime fights never sit together. `intersects` is that
/// predicate. The name is this crate's: the game calls it `EncounterTag`, which
/// says what it is stored as rather than what it means.
#[test]
fn sharing_a_family_is_what_keeps_two_encounters_apart() {
    let slimes = encounter("SLIMES_NORMAL").data().families;
    let weak_slimes = encounter("SLIMES_WEAK").data().families;
    let knights = encounter("KNIGHTS_ELITE").data().families;

    assert!(slimes.contains(Family::Slimes));
    assert!(slimes.intersects(weak_slimes), "both fights are slimes");
    assert!(!slimes.intersects(knights));

    // An encounter with no tags shares nothing, including with itself, so the
    // untagged majority can sit anywhere.
    let untagged = encounter("MYTES_NORMAL").data().families;
    assert_eq!(untagged.iter().count(), 0);
    assert!(!untagged.intersects(untagged));
}

#[test]
fn encounter_seed_salts_are_the_cast_entry_hashes() {
    for encounter in Encounter::ALL {
        assert_eq!(
            encounter.seed_salt(),
            sts2_core::rng::name_hash(&wire(encounter)) as u32,
            "{}",
            wire(encounter)
        );
    }
}

#[test]
fn roster_kind_is_not_an_option() {
    let EncounterRoster::Fixed(monsters) = Encounter::MockMonsterEncounter.roster() else {
        panic!("the mock encounter has a fixed roster");
    };
    assert_eq!(monsters.collect::<Vec<_>>(), [Monster::BigDummy]);
    assert!(matches!(
        Encounter::SlimesWeak.roster(),
        EncounterRoster::Random
    ));
    assert!(matches!(
        Encounter::BattlewornDummyEventEncounter.roster(),
        EncounterRoster::Configured
    ));
}
