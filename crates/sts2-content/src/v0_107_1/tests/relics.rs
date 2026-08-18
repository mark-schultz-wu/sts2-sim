//! The relic table against the dump it came from.
//!
//! Replay the dump row by row, as the other domains do. What is particular here
//! is `Price`, because it is the one column the table does not store the way
//! the dump wrote it: 999999999 is a sentinel and becomes `Price::NotSold`,
//! and the mapping has to be checked in both directions or the table would be
//! free to invent a price nothing charges.

use super::super::relics::*;
use super::super::{RELICS, VERSION};
use super::{build, rows, wire};
use crate::Field;
use std::collections::{BTreeMap, BTreeSet};

/// The sentinel `RelicModel.MerchantCost` returns for a relic not for sale.
///
/// Spelled here as well as in the generator, so that changing it has to be said
/// twice — it is a magic number in the game and a variant in the table, and the
/// only thing tying them together is that both files say it.
const NOT_SOLD: &str = "999999999";

fn relic(id: &str) -> Relic {
    *Relic::ALL
        .iter()
        .find(|r| wire(r) == id)
        .unwrap_or_else(|| panic!("{id} is in the dump but not generated"))
}

/// The anchor: a dump from another build has another hash, and a table
/// regenerated against one is the failure this prevents.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(RELICS),
        VERSION,
        "relics.txt was dumped from a different build than this module claims"
    );
}

#[test]
fn every_relic_is_generated() {
    let dumped: Vec<&str> = rows(RELICS, "relic").map(|r| r[1]).collect();
    assert_eq!(dumped.len(), 297, "the build has 297 relics");
    assert_eq!(Relic::ALL.len(), dumped.len());

    let mut seen = BTreeSet::new();
    for id in dumped {
        assert!(seen.insert(id), "duplicate relic id {id}");
        assert_eq!(wire(&relic(id)), id);
    }
}

/// The dump column each flag comes from.
///
/// Spelled out rather than taken as "the columns after the rarity", because
/// that is the mapping `FLAGS` in the generator decides: `RelicFlags` is a
/// bitset whose numbering is ours, so a member reordered there would move every
/// bit after it without changing a single name.
const FLAG_COLUMNS: [(usize, RelicFlag); 9] = [
    (4, RelicFlag::UsedUp),
    (5, RelicFlag::UponPickup),
    (6, RelicFlag::SpawnsPets),
    (7, RelicFlag::Stackable),
    (8, RelicFlag::AddsPet),
    (9, RelicFlag::AllowedInShops),
    (10, RelicFlag::ShowsCounter),
    (12, RelicFlag::GatedInRun),
    (13, RelicFlag::GatedAtNeow),
];

#[test]
fn the_generated_table_matches_the_dump() {
    let mut checked = 0;

    for row in rows(RELICS, "relic") {
        let id = row[1];
        let d = relic(id).data();

        assert_eq!(wire(&d.rarity), row[2], "{id}: rarity");

        let price = match d.price {
            Price::NotSold => NOT_SOLD.to_string(),
            Price::Gold(n) => n.to_string(),
        };
        assert_eq!(price, row[3], "{id}: price");

        for (column, flag) in FLAG_COLUMNS {
            let cell = match d.flags.get(flag) {
                Field::Computed => "dynamic".to_string(),
                Field::Constant(b) => if b { "1" } else { "0" }.to_string(),
            };
            assert_eq!(cell, row[column], "{id}: {flag:?}");
        }

        let display = match d.display_amount {
            Field::Computed => "dynamic".to_string(),
            Field::Constant(n) => n.to_string(),
        };
        assert_eq!(display, row[11], "{id}: display amount");

        checked += 1;
    }

    assert_eq!(checked, 297);
}

/// Every relic is drafted from at least one pool, and three from two.
///
/// Worth saying because it is what makes the file buckets a partition, and it
/// is not true of the neighbouring domains: 24 monsters are in no act because
/// only a summon brings them in, and 16 encounters are in no act because an
/// event starts them. A relic always comes from somewhere.
#[test]
fn every_relic_is_in_a_pool() {
    let mut dumped: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for row in rows(RELICS, "pool") {
        dumped.entry(row[2]).or_default().push(row[1]);
    }

    let mut crossover: Vec<String> = Vec::new();
    for r in Relic::ALL {
        let id = wire(&r);
        let pools = r.pools();
        assert!(!pools.is_empty(), "{id} is in no pool");

        let want = dumped
            .get(id.as_str())
            .unwrap_or_else(|| panic!("{id} has no pool row"));
        let got: Vec<String> = pools.iter().map(wire).collect();
        assert_eq!(got, *want, "{id}: pools");

        if pools.len() > 1 {
            crossover.push(id);
        }
    }

    assert_eq!(
        crossover,
        ["LASTING_CANDY", "RAZOR_TOOTH", "SPARKLING_ROUGE"],
        "the relics drafted from more than one pool have changed"
    );
}

/// `NotSold` is read per relic, and cannot be derived from rarity.
///
/// This is the assertion the `Price` split rests on. The base `MerchantCost`
/// maps Ancient, Starter and Event to the sentinel, so deriving from rarity
/// looks right — and is wrong for nine relics, because nine override
/// `MerchantCost` outright. All nine are Event rarity, which is why the
/// shortcut would have been invisible: 26 of the 35 Event relics are unsellable
/// and the other 9 have a real price.
#[test]
fn not_sold_is_not_a_function_of_rarity() {
    let mut by_rarity: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for row in rows(RELICS, "relic") {
        let entry = by_rarity.entry(row[2]).or_default();
        if row[3] == NOT_SOLD {
            entry.0 += 1;
        } else {
            entry.1 += 1;
        }
    }

    // (unsellable, priced) per rarity. Event is the row that matters: it is the
    // only rarity with relics on both sides.
    assert_eq!(by_rarity["Ancient"], (100, 0));
    assert_eq!(by_rarity["Starter"], (10, 0));
    assert_eq!(by_rarity["Event"], (26, 9));
    assert_eq!(by_rarity["Common"], (0, 30));
    assert_eq!(by_rarity["Uncommon"], (0, 40));
    assert_eq!(by_rarity["Rare"], (0, 50));
    assert_eq!(by_rarity["Shop"], (0, 30));
    assert_eq!(by_rarity["None"], (0, 2));

    let sold: Vec<String> = Relic::ALL
        .iter()
        .filter(|r| {
            r.data().rarity == RelicRarity::Event && !matches!(r.data().price, Price::NotSold)
        })
        .map(wire)
        .collect();
    assert_eq!(sold.len(), 9, "the Event relics with a price have changed");

    // And the table never spells the sentinel as a number.
    for r in Relic::ALL {
        if let Price::Gold(n) = r.data().price {
            assert!(n < 1000, "{}: {n} is a sentinel, not a price", wire(&r));
        }
    }
}

/// Exactly the relics the game works something out for, and no others.
///
/// `IsUsedUp` and `ShowCounter` are `virtual`, so a future build can make any
/// relic compute either, and a table that generated a constant for one would be
/// wrong in a way nothing else here would notice.
#[test]
fn only_the_known_relics_are_computed() {
    let computed = |column: usize| -> Vec<&str> {
        rows(RELICS, "relic")
            .filter(|r| r[column] == "dynamic")
            .map(|r| r[1])
            .collect()
    };

    assert_eq!(computed(4).len(), 10, "the set computing IsUsedUp changed");
    assert_eq!(
        computed(10).len(),
        18,
        "the set computing ShowCounter changed"
    );

    // `display_amount` is never a constant other than zero: a relic either
    // reports nothing or works the number out. So the `Field<u16>` really only
    // ever holds two shapes, and a build that started storing a literal count
    // would be doing something this table has never seen.
    for row in rows(RELICS, "relic") {
        assert!(
            row[11] == "0" || row[11] == "dynamic",
            "{}: display amount {} is a constant nobody has seen before",
            row[1],
            row[11]
        );
    }

    // Showing a counter implies computing the number, in both the constant and
    // the computed direction.
    for row in rows(RELICS, "relic").filter(|r| r[10] != "0") {
        assert_eq!(row[11], "dynamic", "{}: shows a counter", row[1]);
    }

    // The converse does not hold, and one relic is why. `BONE_TEA` works out an
    // amount while reporting that it shows no counter — so a simulator cannot
    // use `ShowsCounter` to decide whether the amount is worth asking for, and
    // the two stay separate fields rather than collapsing into one optional.
    let quiet: Vec<&str> = rows(RELICS, "relic")
        .filter(|r| r[10] == "0" && r[11] == "dynamic")
        .map(|r| r[1])
        .collect();
    assert_eq!(
        quiet,
        ["BONE_TEA"],
        "the relics computing an amount they do not show have changed"
    );

    // The columns with no computed cell at all. A relic that started computing
    // one of these would still generate, because every flag is a `Field` — so
    // it is worth saying out loud that none does.
    for (column, what) in [
        (5, "HasUponPickupEffect"),
        (6, "SpawnsPets"),
        (7, "IsStackable"),
        (8, "AddsPet"),
        (9, "IsAllowedInShops"),
    ] {
        assert!(
            computed(column).is_empty(),
            "{what} is now computed on {:?}",
            computed(column)
        );
    }
}

/// `RelicRarity::None` keeps its two members and gets a better name.
///
/// Unlike the `None`s dropped from the card, power and encounter enums, this
/// one is a value two relics actually carry — so it stays, and only the Rust
/// spelling changes. The serde rename is what keeps the wire and the dump
/// unchanged, and that is what this checks.
#[test]
fn undrafted_is_the_games_none_under_a_better_name() {
    assert_eq!(wire(&RelicRarity::Undrafted), "None");
    assert_eq!(RelicRarity::Undrafted as u8, 0, "the game's own number");

    let undrafted: Vec<String> = Relic::ALL
        .iter()
        .filter(|r| r.data().rarity == RelicRarity::Undrafted)
        .map(wire)
        .collect();
    assert_eq!(
        undrafted,
        ["CIRCLET", "DEPRECATED_RELIC"],
        "the relics outside the rarity system have changed"
    );

    // Both are alone in a pool of their own, which is the shape of "nothing
    // rolls this": the fallback pool holds only Circlet, and the deprecated
    // pool only the placeholder.
    assert_eq!(relic("CIRCLET").pools(), [RelicPool::Fallback]);
    assert_eq!(relic("DEPRECATED_RELIC").pools(), [RelicPool::Deprecated]);

    // Circlet stacks, because it is what you get when there is nothing left to
    // get. It is one of only two relics that do.
    assert_eq!(
        relic("CIRCLET").data().flags.get(RelicFlag::Stackable),
        Field::Constant(true)
    );
}

/// The wire form round-trips, and it is the only spelling of an id.
#[test]
fn a_relic_round_trips_through_its_wire_form() {
    let dumped: Vec<&str> = rows(RELICS, "relic").map(|r| r[1]).collect();

    for (r, id) in Relic::ALL.iter().zip(&dumped) {
        let json = serde_json::to_string(r).unwrap();
        assert_eq!(json, format!("\"{id}\""), "the rename is not the dump's id");
        assert_eq!(&serde_json::from_str::<Relic>(&json).unwrap(), r);
    }

    assert!(serde_json::from_str::<Relic>("\"NOT_A_RELIC\"").is_err());
    assert!(serde_json::from_str::<Relic>("\"CIRCLE\"").is_err());
}

/// The price never scales with ascension.
///
/// The dump read `MerchantCost` at all eleven levels and emitted a `scalecost`
/// row wherever it moved. It emitted none, which is why `Price` is a flat
/// number rather than a `Scaled` — a measurement, not an assumption, and the
/// same sweep that caught `FAKE_MERCHANT_EVENT_ENCOUNTER` paying flat gold.
#[test]
fn no_relic_changes_price_with_ascension() {
    assert_eq!(rows(RELICS, "scalecost").count(), 0);
}
