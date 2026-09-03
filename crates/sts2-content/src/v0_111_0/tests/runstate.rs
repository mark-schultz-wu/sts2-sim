//! The six run-state enums against the dump they came from.
//!
//! Shorter than the other files here, because there is less to be wrong about.
//! An enum is a list of names and numbers, and the dump is the same list. What
//! the tests are really holding is the two places a number means something
//! beyond being unique. Those are the discriminant, which is the game's own,
//! and `UnplayableReason`'s, which deliberately is not.

use super::super::runstate::*;
use super::super::{RUNSTATE, VERSION};
use super::{build, rows, wire};

/// One enum's rows, as `(discriminant, member)` in dump order.
fn dumped(name: &str) -> Vec<(u8, &'static str)> {
    let members: Vec<(u8, &'static str)> = rows(RUNSTATE, "enum")
        .filter(|r| r[1] == name)
        .map(|r| (r[2].parse().expect("a discriminant"), r[3]))
        .collect();

    assert!(!members.is_empty(), "the dump reflects no {name}");
    members
}

/// What `ALL` says an enum is, in the same shape.
fn generated<T: serde::Serialize + Copy>(all: &[T]) -> Vec<(u8, String)> {
    all.iter()
        .enumerate()
        .map(|(i, m)| (i as u8, wire(m)))
        .collect()
}

/// The anchor: a dump from another build has another hash.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(RUNSTATE),
        VERSION,
        "runstate.txt was dumped from a different build than this module claims"
    );
}

/// Five of the six are the game's enum, member for member and number for
/// number.
///
/// The discriminant is compared and not just the name. These are not ids that
/// only have to be unique. A `MapPointType` crosses the wire as a name, and a
/// simulator stores it as a number. A member inserted in the middle renumbers
/// everything after it while every name still matches.
#[test]
fn the_five_plain_enums_are_the_games_enums() {
    // `ALL` is generated in dump order, so the position is the discriminant.
    // That is checked rather than assumed by comparing against the dump's own
    // number below.
    for (name, all) in [
        ("MapPointType", generated(&MapPointType::ALL)),
        ("CombatSide", generated(&CombatSide::ALL)),
        ("PlayerTurnPhase", generated(&PlayerTurnPhase::ALL)),
        ("RunRngType", generated(&RunRngType::ALL)),
        ("PlayerRngType", generated(&PlayerRngType::ALL)),
    ] {
        let dumped: Vec<(u8, String)> = dumped(name)
            .into_iter()
            .map(|(d, m)| (d, m.to_string()))
            .collect();
        assert_eq!(all, dumped, "{name} is not the game's enum");
    }
}

/// The zeros are kept, because the game sets them.
///
/// Cards drop a C# zero that means "absent" and spell it `Option` instead, and
/// this file deliberately does not. `CombatManager.SetPhaseForAllPlayers`
/// assigns `PlayerTurnPhase::None`, `StandardActMap` builds a grid of
/// `MapPointType::Unassigned`, and both reach the observation as themselves.
///
/// A build that stopped setting one still generates it, so this is a note as
/// much as a test.
#[test]
fn the_zeros_that_other_domains_would_drop() {
    assert_eq!(MapPointType::ALL[0], MapPointType::Unassigned);
    assert_eq!(CombatSide::ALL[0], CombatSide::None);
    assert_eq!(PlayerTurnPhase::ALL[0], PlayerTurnPhase::None);

    // And `UnplayableReason` has no zero at all, which is the exception.
    assert!(!UnplayableReason::ALL.iter().any(|r| wire(r) == "None"));
}

/// `UnplayableReason` is numbered by bit index, and the set carries the value.
///
/// The only place in the crate where the generated discriminant is *not* the
/// game's number. The game's members are bit values, and generating those as
/// discriminants makes `1 << reason` a number no hardware has.
///
/// So the discriminant is the index, and the set's word is the game's `[Flags]`
/// value. This checks both halves against the dump.
#[test]
fn an_unplayable_reason_is_a_bit_index_and_the_set_is_the_games_word() {
    let dumped: Vec<(u8, &str)> = dumped("UnplayableReason")
        .into_iter()
        .filter(|(value, _)| *value != 0)
        .collect();

    assert_eq!(dumped.len(), UnplayableReason::ALL.len());

    for (&reason, (value, name)) in UnplayableReason::ALL.iter().zip(&dumped) {
        assert_eq!(&wire(&reason), name, "the members are in the game's order");
        assert_eq!(
            1u8 << (reason as u8),
            *value,
            "{name} is bit {}, so the game's value should be {value}",
            reason as u8
        );
    }

    // Bit 0 is unused, because the game skips value 1. Nothing else in the
    // crate has a hole at the bottom, so it is worth saying out loud.
    assert!(UnplayableReason::ALL.iter().all(|r| *r as u8 != 0));
    assert_eq!(UnplayableReason::HasUnplayableKeyword as u8, 1);
}

/// A card can fail more than one rule, which is why the set exists.
///
/// `CardModel.CanPlay` starts at `None` and `|=` a bit per rule. So an
/// unaffordable card with no living ally to target carries both. A single
/// choice has to pick one, and lies about the other. Empty is the game's
/// `UnplayableReason.None`, which is what the mod already sends as `null`.
#[test]
fn a_card_can_fail_several_rules_at_once() {
    let both = UnplayableReasons::new(&[
        UnplayableReason::EnergyCostTooHigh,
        UnplayableReason::NoLivingAllies,
    ]);

    assert!(both.contains(UnplayableReason::EnergyCostTooHigh));
    assert!(both.contains(UnplayableReason::NoLivingAllies));
    assert!(!both.contains(UnplayableReason::BlockedByHook));
    assert!(!both.is_empty());

    assert_eq!(
        both.iter().collect::<Vec<_>>(),
        [
            UnplayableReason::EnergyCostTooHigh,
            UnplayableReason::NoLivingAllies
        ],
        "iteration is in the enum's order, not the order they were added"
    );

    // The empty set is the game's `None`, and a playable card has no reasons.
    assert!(UnplayableReasons::new(&[]).is_empty());
    assert_eq!(UnplayableReasons::new(&[]).iter().count(), 0);

    // Every reason at once fits the word, which is what makes the set a `u8`.
    let all = UnplayableReasons::new(&UnplayableReason::ALL);
    assert_eq!(all.iter().count(), UnplayableReason::ALL.len());
    assert_eq!(size_of::<UnplayableReasons>(), 1);
}

/// The wire spelling is the C# member name, which is what the game sends.
///
/// The observation carries all six as strings: `p.PointType.ToString()`,
/// `c.Side.ToString()`, and the RNG counters as dictionary keys. So the rename
/// has to be the member name exactly. It is, because the game's names are
/// already the Rust ones. This is what notices if a build renames a member and
/// codegen quietly follows.
#[test]
fn every_member_is_spelled_the_way_the_game_spells_it() {
    for (name, spellings) in [
        ("MapPointType", MapPointType::ALL.iter().map(wire).collect()),
        ("CombatSide", CombatSide::ALL.iter().map(wire).collect()),
        (
            "PlayerTurnPhase",
            PlayerTurnPhase::ALL.iter().map(wire).collect::<Vec<_>>(),
        ),
        ("RunRngType", RunRngType::ALL.iter().map(wire).collect()),
        (
            "PlayerRngType",
            PlayerRngType::ALL.iter().map(wire).collect(),
        ),
        (
            "UnplayableReason",
            UnplayableReason::ALL.iter().map(wire).collect(),
        ),
    ] {
        let dumped: Vec<&str> = dumped(name)
            .into_iter()
            .map(|(_, m)| m)
            .filter(|m| !(name == "UnplayableReason" && *m == "None"))
            .collect();
        assert_eq!(spellings, dumped, "{name}'s wire spellings");
    }

    // And they parse back, which is the direction the observation goes.
    assert_eq!(
        serde_json::from_str::<MapPointType>("\"RestSite\"").unwrap(),
        MapPointType::RestSite
    );
    assert_eq!(
        serde_json::from_str::<RunRngType>("\"MonsterAi\"").unwrap(),
        RunRngType::MonsterAi
    );
    assert!(serde_json::from_str::<MapPointType>("\"NotAPoint\"").is_err());
}

/// All six vocabularies, pinned by name.
///
/// The replay above cannot notice a build that *changes* one of these, because
/// the dump and the table move together. Every list here is a compile-time
/// array, so the diff is a line a person has to write.
///
/// The streams are the sharpest of the six. A build that renames one turns the
/// counter comparison off, unless it fails here first.
#[test]
fn the_words_this_build_uses() {
    assert_eq!(
        MapPointType::ALL.map(|m| wire(&m)),
        [
            "Unassigned",
            "Unknown",
            "Shop",
            "Treasure",
            "RestSite",
            "Monster",
            "Elite",
            "Boss",
            "Ancient"
        ],
        "the kinds of map node have changed"
    );
    assert_eq!(
        CombatSide::ALL.map(|s| wire(&s)),
        ["None", "Player", "Enemy"],
        "the sides of a fight have changed"
    );
    assert_eq!(
        PlayerTurnPhase::ALL.map(|p| wire(&p)),
        [
            "None",
            "Start",
            "AutoPrePlay",
            "Play",
            "AutoPostPlay",
            "End"
        ],
        "the phases of a turn have changed"
    );
    assert_eq!(
        UnplayableReason::ALL.map(|r| wire(&r)),
        [
            "HasUnplayableKeyword",
            "BlockedByHook",
            "BlockedByCardLogic",
            "EnergyCostTooHigh",
            "StarCostTooHigh",
            "NoLivingAllies"
        ],
        "the rules a card can fail have changed"
    );
    assert_eq!(
        RunRngType::ALL.map(|r| wire(&r)),
        [
            "UpFront",
            "Shuffle",
            "UnknownMapPoint",
            "CombatCardGeneration",
            "CombatPotionGeneration",
            "CombatCardSelection",
            "CombatEnergyCosts",
            "CombatTargets",
            "MonsterAi",
            "Niche",
            "CombatOrbs",
            "TreasureRoomRelics"
        ],
        "a run's random streams have changed"
    );
    assert_eq!(
        PlayerRngType::ALL.map(|r| wire(&r)),
        ["Rewards", "Shops", "Transformations"],
        "a player's random streams have changed"
    );
}
