//! The potion table against the dump it came from.
//! Both game `None` members throw when reached. The table omits one and represents the other as an absent `rarity`. These tests distinguish intentional omissions from accidental ones.

use super::super::cards::TargetType;
use super::super::potions::*;
use super::super::{POTIONS, VERSION};
use super::{build, rows, wire};
use crate::Field;
use std::collections::{BTreeMap, BTreeSet};

fn potion(id: &str) -> Potion {
    *Potion::ALL
        .iter()
        .find(|p| wire(p) == id)
        .unwrap_or_else(|| panic!("{id} is in the dump but not generated"))
}

/// The members `DROPPED` in `xtask/src/codegen/potions.rs`.
const DROPPED: [(&str, &str); 2] = [("PotionUsage", "None"), ("PotionRarity", "None")];

/// The anchor: a dump from another build has another hash.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(POTIONS),
        VERSION,
        "potions.txt was dumped from a different build than this module claims"
    );
}

#[test]
fn the_generated_table_matches_the_dump() {
    let mut checked = 0;

    for row in rows(POTIONS, "potion") {
        let id = row[1];
        let d = potion(id).data();

        let rarity = d.rarity.map_or("None".to_string(), |r| wire(&r));
        assert_eq!(rarity, row[2], "{id}: rarity");
        assert_eq!(wire(&d.usage), row[3], "{id}: usage");

        let target = match d.target {
            Field::Computed => "dynamic".to_string(),
            Field::Constant(t) => wire(&t),
        };
        assert_eq!(target, row[4], "{id}: target");

        for (column, flag) in [
            (5, PotionFlag::GeneratedInCombat),
            (6, PotionFlag::PassesUsabilityCheck),
        ] {
            let cell = match d.flags.get(flag) {
                Field::Computed => "dynamic".to_string(),
                Field::Constant(b) => if b { "1" } else { "0" }.to_string(),
            };
            assert_eq!(cell, row[column], "{id}: {flag:?}");
        }

        checked += 1;
    }

    assert_eq!(checked, 65, "the build has 65 potions");
    assert_eq!(Potion::ALL.len(), 65);
}

/// Every potion is stocked by exactly one pool.
///
/// The reason the buckets are a clean partition. Not something the neighbouring domains manage: a relic can be in two pools, a monster in no act. An event in neither an act nor the pool. A potion is always in one.
#[test]
fn every_potion_is_in_exactly_one_pool() {
    let mut dumped: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for row in rows(POTIONS, "pool") {
        dumped.entry(row[2]).or_default().push(row[1]);
    }

    let mut per_pool: BTreeMap<String, usize> = BTreeMap::new();
    for p in Potion::ALL {
        let id = wire(&p);
        let want = dumped
            .get(id.as_str())
            .unwrap_or_else(|| panic!("{id} has no pool row"));
        assert_eq!(want.len(), 1, "{id} is in {} pools", want.len());
        assert_eq!(wire(&p.pool()), want[0], "{id}: pool");
        *per_pool.entry(wire(&p.pool())).or_default() += 1;
    }

    assert_eq!(per_pool.len(), 10, "the build has 10 potion pools");
    assert_eq!(
        per_pool["SHARED_POTION_POOL"], 45,
        "most potions are in the shared pool"
    );
    for character in ["IRONCLAD", "SILENT", "DEFECT", "REGENT", "NECROBINDER"] {
        assert_eq!(
            per_pool[&format!("{character}_POTION_POOL")],
            3,
            "{character} has three potions of its own"
        );
    }
}

/// `PotionUsage::None` is not generated, and nothing has it.
///
/// `NPotionPopup` throws `"No potions must have 'None' usage."` on the branch that will handle one. That is the game saying the member must not exist, and the build agreeing. Therefore dropping it is safe only as long as both hold, which is what this checks.
#[test]
fn no_potion_has_the_usage_the_game_says_it_should_not() {
    assert!(
        rows(POTIONS, "potion").all(|r| r[3] != "None"),
        "a potion now has None usage"
    );

    // Declared by the game, and absent from the generated enum.
    let declared: Vec<&str> = rows(POTIONS, "enum")
        .filter(|r| r[1] == "PotionUsage")
        .map(|r| r[3])
        .collect();
    assert_eq!(declared, ["None", "CombatOnly", "AnyTime", "Automatic"]);
    assert!(serde_json::from_str::<PotionUsage>("\"None\"").is_err());

    // The three that survive are at the game's own numbers.
    assert_eq!(PotionUsage::CombatOnly as u8, 1);
    assert_eq!(PotionUsage::AnyTime as u8, 2);
    assert_eq!(PotionUsage::Automatic as u8, 3);
}

/// `PotionRarity::None` became an absent rarity, and two models have it.
///
/// Unlike the usage, something does carry this, so it cannot  be dropped. `PotionRarityExtensions.ToLocString` throws on it, so it is not a rarity the game can name, and `Option` is what says that. Both carriers are non-content, which is worth pinning: a real potion turning up without a rarity means the reasoning here no longer holds.
#[test]
fn only_non_content_has_no_rarity() {
    let unrated: Vec<String> = Potion::ALL
        .iter()
        .filter(|p| p.data().rarity.is_none())
        .map(wire)
        .collect();

    assert_eq!(
        unrated,
        ["DEPRECATED_POTION", "MOCK_DISCARD_AND_ADD_SHIVS_POTION"],
        "the potions with no rarity have changed"
    );

    // And they are alone in pools of their own, which is the shape of "not
    // content": nothing else is stocked beside them.
    assert_eq!(
        wire(&potion("DEPRECATED_POTION").pool()),
        "DEPRECATED_POTION_POOL"
    );
    assert_eq!(
        wire(&potion("MOCK_DISCARD_AND_ADD_SHIVS_POTION").pool()),
        "MOCK_POTION_POOL"
    );

    assert!(serde_json::from_str::<PotionRarity>("\"None\"").is_err());
    assert_eq!(PotionRarity::Common as u8, 1, "the game's own number");
}

/// Exactly the potions the game works something out for.
///
/// One potion computes two of its cells, and it is the same one both times. `TargetType` and `PassesCustomUsabilityCheck` are the only members here a potion can decide at the time. Therefore a table that generated a constant for either is wrong invisibly.
#[test]
fn only_foul_potion_is_computed() {
    let computed = |column: usize| -> Vec<&str> {
        rows(POTIONS, "potion")
            .filter(|r| r[column] == "dynamic")
            .map(|r| r[1])
            .collect()
    };

    assert_eq!(computed(4), ["FOUL_POTION"], "the set computing a target");
    assert_eq!(
        computed(6),
        ["FOUL_POTION"],
        "the set computing a usability check"
    );

    // Rarity and usage are read off the IL too, and nothing computes either,     // which is what lets them be plain values rather than a `Field`.
    assert!(computed(2).is_empty(), "a potion now computes its rarity");
    assert!(computed(3).is_empty(), "a potion now computes its usage");
    assert!(
        computed(5).is_empty(),
        "a potion now computes whether it can be made in combat"
    );

    assert!(matches!(
        potion("FOUL_POTION").data().target,
        Field::Computed
    ));
}

/// Three potions cannot be conjured mid-fight.
#[test]
fn three_potions_are_never_made_in_combat() {
    let never: Vec<String> = Potion::ALL
        .iter()
        .filter(|p| p.data().flags.get(PotionFlag::GeneratedInCombat) == Field::Constant(false))
        .map(wire)
        .collect();

    assert_eq!(
        never,
        ["FAIRY_IN_A_BOTTLE", "FRUIT_JUICE", "REGEN_POTION"],
        "the potions that cannot be made in combat have changed"
    );
}

/// A potion targets the way a card does, using the cards domain's own enum.
#[test]
fn a_potion_targets_with_the_card_target_type() {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for p in Potion::ALL {
        if let Field::Constant(t) = p.data().target {
            seen.insert(wire(&t));
        }
    }

    // The five a potion actually uses, out of the ten a card can.
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        ["AllEnemies", "AnyEnemy", "AnyPlayer", "Self"]
    );

    // `Self` is a reserved word, so the cards domain renamed the variant and
    // kept the wire form. A potion inherits both.
    assert_eq!(wire(&TargetType::SelfTarget), "Self");
}

/// The wire form round-trips, and it is the only spelling of an id.
#[test]
fn a_potion_round_trips_through_its_wire_form() {
    let dumped: Vec<&str> = rows(POTIONS, "potion").map(|r| r[1]).collect();

    for (p, id) in Potion::ALL.iter().zip(&dumped) {
        let json = serde_json::to_string(p).unwrap();
        assert_eq!(json, format!("\"{id}\""), "the rename is not the dump's id");
        assert_eq!(&serde_json::from_str::<Potion>(&json).unwrap(), p);
    }

    assert!(serde_json::from_str::<Potion>("\"NOT_A_POTION\"").is_err());
}

/// The dropped members are spelled out here as well as in the generator.
#[test]
fn the_generated_enums_drop_only_what_was_declared_dropped() {
    for row in rows(POTIONS, "enum").filter(|r| r[1].starts_with("Potion")) {
        let (name, member) = (row[1], row[3]);
        let parses = match name {
            "PotionRarity" => {
                serde_json::from_str::<PotionRarity>(&format!("\"{member}\"")).is_ok()
            }
            "PotionUsage" => serde_json::from_str::<PotionUsage>(&format!("\"{member}\"")).is_ok(),
            other => panic!("{other} is not a potion enum"),
        };

        assert_eq!(
            parses,
            !DROPPED.contains(&(name, member)),
            "{name}::{member} parses when it should not, or the reverse"
        );
    }
}
