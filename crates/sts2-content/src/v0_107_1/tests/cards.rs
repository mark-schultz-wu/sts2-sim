//! The card table against the dump it came from.

use super::super::cards::*;
use super::super::{CARDS, VERSION};
use super::{build, rows, wire};
use crate::{Cost, Field};
use sts2_core::CardVars;
use sts2_core::{CardTable, Previewed, Table};

/// The dump's `cost` and `costsX` columns as the `Cost` they generate.
///
/// Spelled out independently of the codegen that produced the table, which
/// otherwise only proves it agrees with itself.
///
/// The two columns are not independent. `CardEnergyCost` sets
/// `Canonical = CostsX ? 0 : canonicalCost`, and treats any negative base as
/// "no cost". So an X-cost card carries no number, and `-1` is a sentinel.
/// Both are asserted here.
fn cost(canonical: &str, costs_x: &str) -> Cost {
    match (costs_x, canonical) {
        ("1", n) => {
            assert_eq!(n, "0", "an X-cost card carries a canonical cost");
            Cost::X
        }
        (_, "-1") => Cost::None,
        (_, n) => Cost::Fixed(n.parse().expect("a non-negative energy cost")),
    }
}

/// The same, for stars.
///
/// A separate reading, because the two pools are not spelled the same.
/// `CanonicalStarCost` is a plain virtual returning `-1`. Nothing zeroes it
/// for an X-star card, so the pairing the energy pool asserts does not hold
/// here.
fn star_cost(canonical: &str, costs_x: &str) -> Cost {
    match (costs_x, canonical) {
        ("1", _) => Cost::X,
        (_, "-1") => Cost::None,
        (_, n) => Cost::Fixed(n.parse().expect("a non-negative star cost")),
    }
}

/// The generated table has to agree with the dump it was generated from. If it
/// does not, `cargo xtask codegen` was not re-run, and the table describes a
/// build nobody is talking to.
#[test]
fn the_generated_table_matches_the_dump() {
    let mut checked = 0;
    for row in rows(CARDS, "card") {
        let card = Cards::ALL
            .iter()
            .find(|c| wire(c) == row[1])
            .unwrap_or_else(|| panic!("{} is in the dump but not generated", row[1]));

        let d = card.data();
        assert_eq!(d.cost, cost(row[6], row[7]), "{}: cost", row[1]);
        assert_eq!(
            d.star_cost,
            star_cost(row[8], row[9]),
            "{}: star_cost",
            row[1]
        );
        assert_eq!(
            d.max_upgrade.to_string(),
            row[10],
            "{}: max_upgrade",
            row[1]
        );
        assert_eq!(format!("{:?}", d.rarity), row[4], "{}: rarity", row[1]);

        // Matched, not converted to an Option: `Constant(TargetType::None)`
        // means "targets nothing" and `Computed` means "ask the card", and
        // an Option would spell both with the word None again.
        //
        // Compared through serde rather than Debug, because
        // `TargetType::Self` is a Rust keyword and generates as
        // `SelfTarget` — only the rename carries the game's own spelling,
        // and that is what the wire depends on.
        match d.kind {
            Field::Computed => assert_eq!(row[3], "dynamic", "{}: kind", row[1]),
            Field::Constant(kind) => assert_eq!(wire(&kind), row[3], "{}: kind", row[1]),
        }
        match d.target {
            Field::Computed => assert_eq!(row[5], "dynamic", "{}: target", row[1]),
            Field::Constant(target) => assert_eq!(wire(&target), row[5], "{}: target", row[1]),
        }

        // Sorted, because the set iterates in the game's declaration order
        // and the dump sorts alphabetically. Neither order is meaningful:
        // `CardKeywordOrder` gives the display order separately, and the
        // game itself stores a `HashSet`.
        let mut keywords: Vec<String> = d.keywords.iter().map(|k| format!("{k:?}")).collect();
        keywords.sort();
        let expected = if row[11] == "-" {
            Vec::new()
        } else {
            row[11].split(',').map(String::from).collect()
        };
        assert_eq!(keywords, expected, "{}: keywords", row[1]);

        checked += 1;
    }
    assert_eq!(checked, 578);
}

/// The anchor. A dump from another build has another hash, and a table
/// silently regenerated against one is the failure this pairing exists to
/// prevent.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(CARDS),
        VERSION,
        "cards.txt was dumped from a different build than this module claims"
    );
}

#[test]
fn every_card_is_present_and_well_formed() {
    let cards: Vec<_> = rows(CARDS, "card").collect();
    assert_eq!(cards.len(), 578, "the build has 578 cards");

    for row in &cards {
        assert_eq!(row.len(), 12, "a card row has twelve fields: {row:?}");
        assert_eq!(row[0], "card");
    }
}

/// The 12 pools are the groups the generated enums nest by, so an unknown
/// one means the grouping is about to be wrong.
#[test]
fn every_card_belongs_to_a_known_pool() {
    const POOLS: [&str; 12] = [
        "IRONCLAD_CARD_POOL",
        "SILENT_CARD_POOL",
        "REGENT_CARD_POOL",
        "NECROBINDER_CARD_POOL",
        "DEFECT_CARD_POOL",
        "COLORLESS_CARD_POOL",
        "CURSE_CARD_POOL",
        "DEPRECATED_CARD_POOL",
        "EVENT_CARD_POOL",
        "QUEST_CARD_POOL",
        "STATUS_CARD_POOL",
        "TOKEN_CARD_POOL",
    ];

    for row in rows(CARDS, "card") {
        assert!(
            POOLS.contains(&row[2]),
            "unknown pool for {}: {}",
            row[1],
            row[2]
        );
    }
}

/// The enums the card fields draw from, reflected from the game rather than
/// inferred from the values in use.
///
/// A member no card happens to use is still a member. An enum missing one fails
/// to decode the first time content grows into it.
#[test]
fn the_field_enums_are_complete() {
    for (name, count) in [
        ("CardType", 7),
        ("CardRarity", 11),
        ("TargetType", 10),
        ("CardKeyword", 8),
    ] {
        let members: Vec<_> = rows(CARDS, "enum").filter(|r| r[1] == name).collect();
        assert_eq!(members.len(), count, "{name} member count changed");

        // Discriminants are the wire and table encoding, so a gap or a
        // reorder is a silent remapping rather than an addition.
        for (i, m) in members.iter().enumerate() {
            assert_eq!(m[2], i.to_string(), "{name} is no longer densely numbered");
        }
    }
}

/// Three members the game declares are deliberately not generated. They are the
/// C# zero rather than values of the field they name. See `DROPPED` in the
/// generator for the evidence per member.
///
/// Dropping one is only sound while no card carries it, so that is what this
/// pins. The converse is not asserted, because a generated member no card uses
/// is normal.
#[test]
fn no_card_uses_a_member_that_is_not_generated() {
    for (column, member) in [(3, "CardType::None"), (4, "CardRarity::None")] {
        let uses: Vec<&str> = rows(CARDS, "card")
            .filter(|r| r[column] == "None")
            .map(|r| r[1])
            .collect();
        assert!(uses.is_empty(), "{member} is on {uses:?}");
    }

    let keywords: Vec<&str> = rows(CARDS, "card")
        .filter(|r| r[9].split(',').any(|k| k == "None"))
        .map(|r| r[1])
        .collect();
    assert!(keywords.is_empty(), "CardKeyword::None is on {keywords:?}");
}

/// Ids are the enum variants and the wire form, so a duplicate makes two
/// different cards the same card.
#[test]
fn card_ids_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for row in rows(CARDS, "card") {
        assert!(seen.insert(row[1]), "duplicate card id {}", row[1]);
    }
}

/// `max_upgrade` is a `u8`, so a build that grew a negative or a large one
/// truncates silently. It is 0 or 1 on every shipped card. `CardModel` has a
/// real `if (MaxUpgradeLevel > 1)` branch, so the field is a count rather than a
/// flag, and this is the check that it stays small.
#[test]
fn upgrade_levels_stay_in_a_byte() {
    let mut seen: Vec<&str> = rows(CARDS, "card").map(|r| r[10]).collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen, ["0", "1"], "max upgrade levels changed");
}

/// Exactly the four cards that compute a field, and no others. If a future
/// build adds one, this fails rather than the simulator silently reading a
/// constant that is not one.
#[test]
fn only_the_known_cards_are_dynamic() {
    let dynamic: Vec<&str> = rows(CARDS, "card")
        .filter(|r| r[3] == "dynamic" || r[5] == "dynamic")
        .map(|r| r[1])
        .collect();

    assert_eq!(
        dynamic,
        ["MAD_SCIENCE", "MALAISE", "SHIV", "SOVEREIGN_BLADE"],
        "the set of cards computing Type or TargetType has changed"
    );
}

/// The numbers on a card's face, against the dump they came from.
///
/// A card's own rows, matched by id rather than by position, because the
/// generated table sorts by pool and the dump does not.
#[test]
fn every_printed_number_matches_the_dump() {
    let mut dumped: std::collections::HashMap<&str, Vec<(&str, i32, bool, Previewed)>> =
        std::collections::HashMap::new();
    for row in rows(CARDS, "var") {
        assert_eq!(row.len(), 6, "a var row has six fields: {row:?}");
        dumped.entry(row[1]).or_default().push((
            row[2],
            row[3].parse().expect("a printed number is an integer"),
            row[4] == "1",
            // The hooks a number runs through are the var class's, and the
            // generated table carries what the class means rather than its
            // name. This is the same reading, from the other side.
            match row[5] {
                "DamageVar" | "CalculatedDamageVar" => Previewed::Damage,
                "OstyDamageVar" => Previewed::OstyDamage,
                "BlockVar" | "CalculatedBlockVar" => Previewed::Block,
                _ => Previewed::Printed,
            },
        ));
    }
    assert_eq!(dumped.values().map(Vec::len).sum::<usize>(), 775);

    for card in Cards::ALL {
        let want = dumped.remove(wire(&card).as_str()).unwrap_or_default();
        let got: Vec<(&str, i32, bool, Previewed)> = card
            .vars()
            .iter()
            .map(|v| (v.id.name(), v.base, v.calculated, v.previewed))
            .collect();
        assert_eq!(got, want, "{}: printed numbers", wire(&card));
    }

    assert!(
        dumped.is_empty(),
        "the dump has numbers for cards that are not generated: {:?}",
        dumped.keys().collect::<Vec<_>>()
    );
}

/// The one card priced in stars with an X, and the 21 with a number.
///
/// `Cost::X` is reached through a different column from the energy pool's.
/// Stardust carries the `-1` sentinel beside it, where an X-energy card
/// carries a zero. A build that stops overriding `HasStarCostX` generates
/// `Cost::None` for it, and nothing else here says so.
#[test]
fn only_the_regent_is_priced_in_stars() {
    let priced: Vec<&str> = rows(CARDS, "card")
        .filter(|r| r[8] != "-1" || r[9] == "1")
        .map(|r| r[1])
        .collect();
    assert_eq!(priced.len(), 22, "cards with a star cost: {priced:?}");
    assert!(
        priced
            .iter()
            .all(|id| rows(CARDS, "card").any(|r| r[1] == *id && r[2] == "REGENT_CARD_POOL")),
        "a card outside the Regent's pool is priced in stars: {priced:?}"
    );

    let x: Vec<&str> = rows(CARDS, "card")
        .filter(|r| r[9] == "1")
        .map(|r| r[1])
        .collect();
    assert_eq!(x, ["STARDUST"]);
    for id in x {
        let card = Cards::ALL
            .iter()
            .find(|c| wire(c) == id)
            .expect("an X-star card is generated");
        assert_eq!(card.star_cost(), Cost::X, "{id}: star cost");
    }
}
