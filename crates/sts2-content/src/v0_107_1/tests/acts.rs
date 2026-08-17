//! The act table against the dump it came from.
//!
//! The smallest domain, and the one three others point at. Most of what is
//! worth checking here is not the table's own rows but the agreement between
//! this dump and theirs: `Act` is declared once now, and every domain that
//! names an act has to name one this dump declares.

use super::super::acts::*;
use super::super::{ACTS, ENCOUNTERS, EVENTS, MONSTERS, VERSION};
use super::{dump_hash, rows, wire};
use std::collections::BTreeSet;

fn act(id: &str) -> Act {
    *Act::ALL
        .iter()
        .find(|a| wire(a) == id)
        .unwrap_or_else(|| panic!("{id} is in the dump but not generated"))
}

/// The anchor: a dump from another build has another hash.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        dump_hash(ACTS),
        VERSION.content_hash,
        "acts.txt was dumped from a different build than this module claims"
    );
}

#[test]
fn the_generated_table_matches_the_dump() {
    let mut checked = 0;

    for row in rows(ACTS, "act") {
        let id = row[1];
        let d = act(id).data();

        assert_eq!(d.index.to_string(), row[2], "{id}: index");
        assert_eq!(d.is_default, row[3] == "1", "{id}: is default");
        assert_eq!(d.rooms.to_string(), row[4], "{id}: rooms");
        assert_eq!(
            d.weak_encounters.to_string(),
            row[5],
            "{id}: weak encounters"
        );

        checked += 1;
    }

    assert_eq!(checked, 5, "the build has 5 acts");
    assert_eq!(Act::ALL.len(), 5);
}

/// `index` is not a unique key, and two acts share one.
///
/// `OVERGROWTH` and `UNDERDOCKS` are both 0: they are the two alternative first
/// acts, and `is_default` is what picks between them. Worth pinning because the
/// obvious modelling — index as a run position — is wrong about exactly the
/// place a run starts, and would be right about everything after it.
#[test]
fn two_acts_share_the_first_position() {
    let first: Vec<String> = Act::ALL
        .iter()
        .filter(|a| a.data().index == 0)
        .map(wire)
        .collect();
    assert_eq!(first, ["OVERGROWTH", "UNDERDOCKS"]);

    assert!(act("OVERGROWTH").data().is_default);
    assert!(!act("UNDERDOCKS").data().is_default);

    // `is_default` is per index, not per run: it marks the canonical act for
    // its own position, so three acts have it and the alternative first act
    // does not. Reading it as "where a run starts" would make three acts the
    // opening one.
    for index in 0..=2 {
        let defaults: Vec<String> = Act::ALL
            .iter()
            .filter(|a| a.data().index == index && a.data().is_default)
            .map(wire)
            .collect();
        assert_eq!(defaults.len(), 1, "index {index} has one canonical act");
    }

    assert_eq!(
        Act::ALL
            .iter()
            .filter(|a| a.data().is_default)
            .map(wire)
            .collect::<Vec<_>>(),
        ["OVERGROWTH", "HIVE", "GLORY"]
    );
}

/// The deprecated act is the one a run never visits, and -1 is how it says so.
///
/// It is reflected in like every other domain's placeholders rather than left
/// out, so the enum spans the build. Nothing else should name it.
#[test]
fn the_deprecated_act_is_the_only_one_outside_a_run() {
    let outside: Vec<String> = Act::ALL
        .iter()
        .filter(|a| a.data().index < 0)
        .map(wire)
        .collect();
    assert_eq!(outside, ["DEPRECATED_ACT"]);

    let d = act("DEPRECATED_ACT").data();
    assert_eq!(d.rooms, 0, "it has no map");
    assert_eq!(d.bosses.len(), 0, "and no bosses");

    // No other dump names it, which is what makes it safe to have in the enum.
    for dump in [MONSTERS, ENCOUNTERS, EVENTS] {
        assert!(
            rows(dump, "act").all(|r| r[1] != "DEPRECATED_ACT"),
            "a domain now places content in the deprecated act"
        );
    }
}

/// Every act a run visits reveals three bosses, in order.
///
/// The order is the rule and not presentation: `BossDiscoveryOrder` is the
/// sequence a player meets an act's bosses in across runs, so a reordering
/// changes which boss a first run ends against.
#[test]
fn each_act_reveals_its_bosses_in_order() {
    let mut dumped: Vec<(&str, Vec<&str>)> = Vec::new();
    for row in rows(ACTS, "boss") {
        match dumped.iter_mut().find(|(id, _)| *id == row[1]) {
            Some((_, bosses)) => bosses.push(row[3]),
            None => dumped.push((row[1], vec![row[3]])),
        }
    }

    for a in Act::ALL {
        let id = wire(&a);
        let got: Vec<String> = a.data().bosses.iter().map(|b| wire(&b)).collect();
        let want = dumped
            .iter()
            .find(|(dumped_id, _)| *dumped_id == id)
            .map(|(_, b)| b.clone())
            .unwrap_or_default();

        assert_eq!(got, want, "{id}: bosses");
        if a.data().index >= 0 {
            assert_eq!(got.len(), 3, "{id} reveals three bosses");
        }
    }

    assert_eq!(
        act("OVERGROWTH")
            .data()
            .bosses
            .iter()
            .map(|b| wire(&b))
            .collect::<Vec<_>>(),
        ["VANTOM_BOSS", "CEREMONIAL_BEAST_BOSS", "THE_KIN_BOSS"]
    );
}

/// Every act another domain names is one this dump declares.
///
/// `Act` used to be generated by the monsters domain from whichever ids turned
/// up in its own dump, which meant three domains agreed by construction rather
/// than by check. Now it is declared here, and this is the check: the acts
/// naming content are exactly the four a run visits.
#[test]
fn every_domain_names_an_act_this_dump_declares() {
    let declared: BTreeSet<String> = Act::ALL.iter().map(wire).collect();

    let mut used: BTreeSet<&str> = BTreeSet::new();
    for dump in [MONSTERS, ENCOUNTERS, EVENTS] {
        for row in rows(dump, "act") {
            assert!(
                declared.contains(row[1]),
                "{} is named as an act but is not one",
                row[1]
            );
            used.insert(row[1]);
        }
    }

    assert_eq!(
        used.into_iter().collect::<Vec<_>>(),
        ["GLORY", "HIVE", "OVERGROWTH", "UNDERDOCKS"],
        "the acts that hold content have changed"
    );
}

/// Rooms shrink as a run goes on.
#[test]
fn an_act_gets_shorter_the_later_it_is() {
    let mut visited: Vec<(i8, u8)> = Act::ALL
        .iter()
        .map(|a| (a.data().index, a.data().rooms))
        .filter(|(index, _)| *index >= 0)
        .collect();
    visited.sort_unstable();

    assert_eq!(visited, [(0, 15), (0, 15), (1, 14), (2, 13)]);

    // And the weak-encounter count drops once, at the same point.
    assert_eq!(act("OVERGROWTH").data().weak_encounters, 3);
    assert_eq!(act("HIVE").data().weak_encounters, 2);
}

/// The wire form round-trips, and it is the only spelling of an id.
#[test]
fn an_act_round_trips_through_its_wire_form() {
    let dumped: Vec<&str> = rows(ACTS, "act").map(|r| r[1]).collect();

    for (a, id) in Act::ALL.iter().zip(&dumped) {
        let json = serde_json::to_string(a).unwrap();
        assert_eq!(json, format!("\"{id}\""), "the rename is not the dump's id");
        assert_eq!(&serde_json::from_str::<Act>(&json).unwrap(), a);
    }

    assert!(serde_json::from_str::<Act>("\"NOT_AN_ACT\"").is_err());
}
