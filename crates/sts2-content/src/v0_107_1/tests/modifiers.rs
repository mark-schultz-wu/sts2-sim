//! The modifier table against the dump it came from.
//!
//! Seventeen rows and almost no columns, because a modifier is its hooks and
//! hooks are code. What the tests hold are the three facts a caller needs before
//! any ported hook runs: which list a run offers it from, what picking it rules
//! out, and what it changes about setting the run up.

use super::super::modifiers::*;
use super::super::{MODIFIERS, VERSION};
use super::{dump_hash, rows, wire};

fn modifier(id: &str) -> Modifier {
    *Modifier::ALL
        .iter()
        .find(|m| wire(m) == id)
        .unwrap_or_else(|| panic!("{id} is in the dump but not generated"))
}

/// The anchor: a dump from another build has another hash.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        dump_hash(MODIFIERS),
        VERSION.content_hash,
        "modifiers.txt was dumped from a different build than this module claims"
    );
}

#[test]
fn the_generated_table_matches_the_dump() {
    let mut checked = 0;

    for row in rows(MODIFIERS, "modifier") {
        let id = row[1];
        let d = modifier(id).data();

        let kind = d.kind.map(|k| wire(&k)).unwrap_or("unused".to_string());
        assert_eq!(kind, row[2], "{id}: kind");

        for (column, flag) in [
            (3, ModifierFlag::ClearsDeck),
            (4, ModifierFlag::OffersNeowOption),
            (5, ModifierFlag::TakesParameter),
        ] {
            let set = if d.flags.contains(flag) { "1" } else { "0" };
            assert_eq!(set, row[column], "{id}: {flag:?}");
        }

        checked += 1;
    }

    assert_eq!(checked, 17, "the build has 17 modifiers");
    assert_eq!(Modifier::ALL.len(), 17);
}

/// Nine good, seven bad, and one a run cannot offer at all.
///
/// The two lists are the whole of what a run picks from, so a modifier in
/// neither is unreachable — the deprecated one, which is here for the same
/// reason `DEPRECATED_ACT` is: the enum spans the build rather than the
/// playable part of it.
#[test]
fn the_deprecated_modifier_is_the_only_one_no_run_can_offer() {
    let of = |kind: Option<ModifierKind>| -> Vec<String> {
        Modifier::ALL
            .iter()
            .filter(|m| m.data().kind == kind)
            .map(wire)
            .collect()
    };

    assert_eq!(of(None), ["DEPRECATED_MODIFIER"]);
    assert_eq!(of(Some(ModifierKind::Good)).len(), 9);
    assert_eq!(of(Some(ModifierKind::Bad)).len(), 7);

    assert_eq!(
        of(Some(ModifierKind::Bad)),
        [
            "BIG_GAME_HUNTER",
            "CURSED_RUN",
            "DEADLY_EVENTS",
            "MIDAS",
            "MURDEROUS",
            "NIGHT_TERRORS",
            "TERMINAL"
        ],
        "which modifiers are drawbacks has changed"
    );
}

/// The one exclusion group is exactly the three that clear the deck.
///
/// Two facts the game states separately — `MutuallyExclusiveModifiers` and three
/// overrides of `ClearsPlayerDeck` — and they coincide, which is the sort of
/// thing worth pinning rather than relying on. If a build ever adds a
/// deck-clearing modifier outside the group, or a group member that keeps the
/// deck, this says so instead of the coincidence quietly becoming an assumption
/// somebody coded against.
#[test]
fn the_exclusion_group_is_the_deck_clearing_modifiers() {
    assert_eq!(EXCLUSIVE.len(), 1, "one group in this build");

    let clears: Vec<String> = Modifier::ALL
        .iter()
        .filter(|m| m.data().flags.contains(ModifierFlag::ClearsDeck))
        .map(wire)
        .collect();
    assert_eq!(clears, ["DRAFT", "INSANITY", "SEALED_DECK"]);

    let group: Vec<String> = EXCLUSIVE[0].iter().map(|m| wire(&m)).collect();
    assert_eq!(group, clears, "the group and the deck-clearers have parted");

    // And the dump agrees, which is what makes this a check on the game rather
    // than on one projection of it.
    let dumped: Vec<&str> = rows(MODIFIERS, "exclusive").map(|r| r[2]).collect();
    assert_eq!(dumped, ["DRAFT", "INSANITY", "SEALED_DECK"]);
}

/// `intersects` is the question a caller actually has.
///
/// A group is a set rather than a pair because the game holds a set, so the
/// useful operation is "does what I have already chosen conflict with this"
/// rather than "is this the other one".
#[test]
fn a_second_modifier_from_a_group_is_what_the_set_rules_out() {
    let group = EXCLUSIVE[0];

    assert!(group.contains(Modifier::Draft));
    assert!(!group.contains(Modifier::Hoarder));

    let chosen = ModifierSet::new(&[Modifier::Draft, Modifier::Hoarder]);
    assert!(group.intersects(chosen), "Draft is already in the group");

    let clean = ModifierSet::new(&[Modifier::Hoarder, Modifier::Flight]);
    assert!(!group.intersects(clean));
    assert!(ModifierSet::new(&[]).is_empty());
    assert!(!group.is_empty());
}

/// Five modifiers meet the player at Neow, and one carries run state.
#[test]
fn the_flags_are_as_rare_as_they_look() {
    let with = |flag: ModifierFlag| -> Vec<String> {
        Modifier::ALL
            .iter()
            .filter(|m| m.data().flags.contains(flag))
            .map(wire)
            .collect()
    };

    assert_eq!(
        with(ModifierFlag::OffersNeowOption),
        [
            "ALL_STAR",
            "DRAFT",
            "INSANITY",
            "SEALED_DECK",
            "SPECIALIZED"
        ]
    );

    // `CHARACTER_CARDS` names a character, and which one is run state — the
    // table can say there is a parameter and not what it is.
    assert_eq!(with(ModifierFlag::TakesParameter), ["CHARACTER_CARDS"]);
    assert_eq!(
        modifier("CHARACTER_CARDS").data().kind,
        Some(ModifierKind::Good)
    );
}

/// The wire form round-trips, and it is the only spelling of an id.
#[test]
fn a_modifier_round_trips_through_its_wire_form() {
    let dumped: Vec<&str> = rows(MODIFIERS, "modifier").map(|r| r[1]).collect();

    for (m, id) in Modifier::ALL.iter().zip(&dumped) {
        let json = serde_json::to_string(m).unwrap();
        assert_eq!(json, format!("\"{id}\""), "the rename is not the dump's id");
        assert_eq!(&serde_json::from_str::<Modifier>(&json).unwrap(), m);
    }

    assert!(serde_json::from_str::<Modifier>("\"NOT_A_MODIFIER\"").is_err());
    assert_eq!(wire(&ModifierKind::Good), "good");
    assert_eq!(wire(&ModifierKind::Bad), "bad");
}
