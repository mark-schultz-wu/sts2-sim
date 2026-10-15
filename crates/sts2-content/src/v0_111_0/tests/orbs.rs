//! The orb table against the dump it came from.
//!
//! The smallest table with the sharpest trap in it. There are two lists of orbs
//! in this build: `ModelDb.Orbs` with four, and `OrbModel._validOrbs` with
//! five. Only the second is what a random channel rolls against. The table
//! generates the second and the dump records both, so the tests below are
//! mostly about that difference staying visible.
//!
//! The values themselves are plain numbers, and the tests hold them to that.
//! The dump measures one thing beyond the number, which is whether a value
//! lives in a field the orb keeps. That is deliberately not generated, so it is
//! pinned here from the dump. It is a real fact about the game that a build can
//! change, and the only place it is noticed is a test.

use super::super::orbs::*;
use super::super::{ORBS, VERSION};
use super::{build, rows, wire};
use nonmax::NonMaxU8;
use sts2_core::Table as _;

fn orb(id: &str) -> Orb {
    *Orb::ALL
        .iter()
        .find(|o| wire(o) == id)
        .unwrap_or_else(|| panic!("{id} is in the dump but not generated"))
}

/// One value row from the dump, by orb and which of the two it is.
fn value(id: &str, which: &str) -> Vec<&'static str> {
    rows(ORBS, "value")
        .find(|r| r[1] == id && r[2] == which)
        .unwrap_or_else(|| panic!("{id} has no {which} value"))
}

/// The anchor: a dump from another build has another hash.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(ORBS),
        VERSION,
        "orbs.txt was dumped from a different build than this module claims"
    );
}

#[test]
fn the_generated_table_matches_the_dump() {
    let mut checked = 0;

    for row in rows(ORBS, "orb") {
        let id = row[1];
        let d = orb(id).data();

        let trigger = d.trigger.map(|t| wire(&t)).unwrap_or("none".to_string());
        assert_eq!(trigger, row[2], "{id}: trigger");

        for (which, got) in [("passive", d.passive), ("evoke", d.evoke)] {
            let dumped = value(id, which);

            let amount = match got.amount {
                Some(n) => n.to_string(),
                None => "dynamic".to_string(),
            };

            assert_eq!(amount, dumped[3], "{id}: {which} amount");
            assert_eq!(got.hooked, dumped[4] == "1", "{id}: {which} hooked");
        }

        checked += 1;
    }

    assert_eq!(checked, 7, "the build has 7 orbs");
    assert_eq!(Orb::ALL.len(), 7);
}

/// `ROLLABLE` is `_validOrbs`, in the order a roll indexes it.
///
/// `GetRandomOrb` is `rng.NextItem(_validOrbs)`, so a reordering of that array
/// changes which orb a given seed channels. The order is checked against the
/// dump's index column rather than against the enum, because the enum is
/// alphabetical and the array is not.
#[test]
fn rollable_is_the_array_a_roll_indexes() {
    let mut dumped: Vec<(usize, &str)> = rows(ORBS, "orb")
        .filter(|r| r[3] != "-1")
        .map(|r| (r[3].parse().expect("an index"), r[1]))
        .collect();
    dumped.sort_unstable();

    assert_eq!(
        dumped.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        ROLLABLE.iter().map(wire).collect::<Vec<_>>()
    );
    assert_eq!(
        ROLLABLE.map(|o| wire(&o)),
        [
            "LIGHTNING_ORB",
            "FROST_ORB",
            "DARK_ORB",
            "PLASMA_ORB",
            "GLASS_ORB"
        ],
        "the order a roll indexes the orbs in has changed"
    );
}

/// The dead list is a subset of the live one, which is what makes it harmless.
///
/// `ModelDb.Orbs` names four of the five and nothing that plays reads it, so it
/// is not generated. That is only safe while every orb it names is one a roll
/// also produces. Glass is the difference today: rollable, and not listed.
#[test]
fn the_list_that_is_not_generated_names_nothing_new() {
    let listed: Vec<&str> = rows(ORBS, "orb")
        .filter(|r| r[4] == "1")
        .map(|r| r[1])
        .collect();
    let rollable: Vec<String> = ROLLABLE.iter().map(wire).collect();

    for id in &listed {
        assert!(
            rollable.iter().any(|r| r == id),
            "{id} is in ModelDb.Orbs but no roll produces it"
        );
    }

    assert_eq!(listed.len(), 4);
    assert_eq!(
        ROLLABLE
            .iter()
            .map(wire)
            .filter(|r| !listed.contains(&r.as_str()))
            .collect::<Vec<_>>(),
        ["GLASS_ORB"],
        "which orb the two lists disagree about has changed"
    );
}

/// Plasma fires a turn earlier than everything else.
///
/// Four orbs trigger before the turn ends and Plasma triggers after the turn
/// starts, which is when its energy is spendable. The mock has no passive at
/// all, and that is what `None` means here.
#[test]
fn only_plasma_pays_out_at_the_start_of_a_turn() {
    let with = |trigger: Option<OrbTrigger>| -> Vec<String> {
        Orb::ALL
            .iter()
            .filter(|o| o.data().trigger == trigger)
            .map(wire)
            .collect()
    };

    assert_eq!(with(Some(OrbTrigger::TurnStart)), ["PLASMA_ORB"]);
    assert_eq!(
        with(Some(OrbTrigger::TurnEnd)),
        [
            "DARK_ORB",
            "FROST_ORB",
            "GLASS_ORB",
            "LIGHTNING_ORB",
            "MOCK_TURN_END_SUSPEND_ORB",
        ]
    );
    assert_eq!(with(None), ["MOCK_COMBAT_CLEANUP_ORB"]);
}

/// The ten numbers, and the one getter that names none.
///
/// Glass's evoke is the only `None`, and the point is that it is not a special
/// orb. Plasma's evoke is twice its passive too, and has a number here because
/// the game spells it as a numeral. The table records what the getter names.
#[test]
fn only_glasss_evoke_names_no_number() {
    let amounts: Vec<(String, Option<u8>, Option<u8>)> = Orb::ALL
        .iter()
        .map(|o| {
            let d = o.data();
            (
                wire(o),
                d.passive.amount.map(|n| n.get()),
                d.evoke.amount.map(|n| n.get()),
            )
        })
        .collect();

    assert_eq!(
        amounts,
        [
            ("DARK_ORB".to_string(), Some(6), Some(6)),
            ("FROST_ORB".to_string(), Some(2), Some(5)),
            ("GLASS_ORB".to_string(), Some(4), None),
            ("LIGHTNING_ORB".to_string(), Some(3), Some(8)),
            ("MOCK_COMBAT_CLEANUP_ORB".to_string(), Some(0), Some(0)),
            ("MOCK_TURN_END_SUSPEND_ORB".to_string(), Some(0), Some(0)),
            ("PLASMA_ORB".to_string(), Some(1), Some(2)),
        ],
        "an orb value has changed"
    );

    // Plasma's evoke is twice its passive and carries a number; Glass's evoke is
    // twice its passive and does not. That difference is the C# spelling, and
    // the table is right to make nothing of it.
    let plasma = orb("PLASMA_ORB").data();
    assert_eq!(
        plasma.evoke.amount.map(|n| n.get()),
        plasma.passive.amount.map(|n| n.get() * 2)
    );
}

/// Two values live in a field the orb keeps, and that is not generated.
///
/// The dump measures it because a build moving a value between a field and a
/// literal changes behaviour. The number stays the same, and Glass's passive
/// stops decaying.
///
/// The table has no column for it, because sorting the values that way sorted
/// the game's source rather than the game. So this is where the claim lives.
#[test]
fn the_two_values_the_orbs_keep_in_a_field() {
    let stored: Vec<(&str, &str)> = rows(ORBS, "value")
        .filter(|r| r[5] == "1")
        .map(|r| (r[1], r[2]))
        .collect();

    assert_eq!(
        stored,
        [("DARK_ORB", "evoke"), ("GLASS_ORB", "passive")],
        "which orb values live in a field has changed"
    );

    // And the numbers beside them are those fields' initialisers, which is what
    // makes them worth generating at all: they are where each orb's own counter
    // begins.
    assert_eq!(orb("DARK_ORB").data().evoke.amount, NonMaxU8::new(6));
    assert_eq!(orb("GLASS_ORB").data().passive.amount, NonMaxU8::new(4));
}

/// Which values Focus reaches.
///
/// `hooked` is not an abstract "something modifies this". Two models implement
/// `ModifyOrbValue`, so the six rows below are exactly the values Focus adds to.
///
/// The ones missing are Plasma's energy at either end, which is the surprising
/// half, and Dark's evoke, which feels Focus through its passive.
#[test]
fn focus_does_not_reach_plasmas_energy() {
    let hooked: Vec<(String, &str)> = Orb::ALL
        .iter()
        .flat_map(|o| {
            let d = o.data();
            [("passive", d.passive.hooked), ("evoke", d.evoke.hooked)]
                .into_iter()
                .filter(|(_, hooked)| *hooked)
                .map(move |(which, _)| (wire(o), which))
                .collect::<Vec<_>>()
        })
        .collect();

    assert_eq!(
        hooked,
        [
            ("DARK_ORB".to_string(), "passive"),
            ("FROST_ORB".to_string(), "passive"),
            ("FROST_ORB".to_string(), "evoke"),
            ("GLASS_ORB".to_string(), "passive"),
            ("LIGHTNING_ORB".to_string(), "passive"),
            ("LIGHTNING_ORB".to_string(), "evoke"),
        ],
        "which orb values Focus reaches has changed"
    );

    let plasma = orb("PLASMA_ORB").data();
    assert!(!plasma.passive.hooked && !plasma.evoke.hooked);

    // Glass's evoke is unhooked and still moves with Focus, because it doubles a
    // hooked passive. `hooked` is about this getter, not about the value's fate.
    assert!(!orb("GLASS_ORB").data().evoke.hooked);
    assert!(orb("GLASS_ORB").data().passive.hooked);
}

/// The table is as small as the types promise.
///
/// `OrbValue` is an optional number and one flag. It fits in two bytes because
/// the absence lives in `NonMaxU8`'s niche rather than in a discriminant beside
/// it. Worth asserting rather than assuming. An `Option<u8>` here is the same
/// code and a bigger table, and nothing else says so.
#[test]
fn a_value_is_a_byte_for_the_number_and_a_byte_for_the_flag() {
    assert_eq!(size_of::<OrbValue>(), 2);
    assert_eq!(size_of::<OrbData>(), 5, "a trigger and two values");
    assert_eq!(
        size_of::<Option<OrbTrigger>>(),
        1,
        "the trigger is a byte too"
    );
}

/// The wire form round-trips, and it is the only spelling of an id.
#[test]
fn an_orb_round_trips_through_its_wire_form() {
    let dumped: Vec<&str> = rows(ORBS, "orb").map(|r| r[1]).collect();

    for (o, id) in Orb::ALL.iter().zip(&dumped) {
        let json = serde_json::to_string(o).unwrap();
        assert_eq!(json, format!("\"{id}\""), "the rename is not the dump's id");
        assert_eq!(&serde_json::from_str::<Orb>(&json).unwrap(), o);
    }

    assert!(serde_json::from_str::<Orb>("\"NOT_AN_ORB\"").is_err());

    // The trigger's renames are the dump's spellings too, so the table above
    // compares like with like.
    assert_eq!(wire(&OrbTrigger::TurnEnd), "turnEnd");
    assert_eq!(wire(&OrbTrigger::TurnStart), "turnStart");
}
