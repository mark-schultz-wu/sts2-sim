//! The ascension ladder, against the dump it came from.
//!
//! Small, and load-bearing out of proportion to its size. Every scaled value in
//! the monster and encounter tables is a threshold on this ladder, stored as a
//! bare discriminant. If the ladder reorders and these tables do not, every one
//! of those thresholds silently means a different rule.

use super::super::ascension::*;
use super::super::{ASCENSION, VERSION};
use super::{build, rows, wire};

/// The anchor: a dump from another build has another ladder.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(ASCENSION),
        VERSION,
        "ascension.txt was dumped from a different build than this module claims"
    );
}

/// Eleven levels, dense and in the game's order.
///
/// Dense is the whole model. `has` is `>=` on the discriminant, so a gap or a
/// reorder makes it quietly wrong rather than failing. Codegen refuses a
/// non-dense ladder, and this says the same from the dump's side.
#[test]
fn the_ladder_is_dense_and_in_order() {
    let dumped: Vec<(u8, &str)> = rows(ASCENSION, "level")
        .map(|r| (r[1].parse().expect("a level"), r[2]))
        .collect();

    assert_eq!(dumped.len(), 11, "the build has 11 ascension levels");
    assert_eq!(AscensionLevel::ALL.len(), 11);

    for (at, (value, name)) in dumped.iter().enumerate() {
        assert_eq!(usize::from(*value), at, "{name} is out of position");
        assert_eq!(wire(&AscensionLevel::ALL[at]), *name);
        assert_eq!(AscensionLevel::ALL[at] as u8, *value);
    }

    assert_eq!(
        dumped.iter().map(|(_, n)| *n).collect::<Vec<_>>(),
        [
            "None",
            "SwarmingElites",
            "WearyTraveler",
            "Poverty",
            "TightBelt",
            "AscendersBane",
            "Inflation",
            "Scarcity",
            "ToughEnemies",
            "DeadlyEnemies",
            "DoubleBoss"
        ],
        "the ladder has changed, and every scaled threshold now means something else"
    );
}

/// Ascension is a scalar, so a level carries everything below it.
///
/// `AscensionManager.HasLevel` is `_level >= (int)level`. That is why the two
/// enemy levels the monster tables key on, `ToughEnemies` and `DeadlyEnemies`,
/// are thresholds and not switches.
#[test]
fn a_level_has_every_rule_below_it() {
    let deadly = AscensionLevel::DeadlyEnemies;

    assert!(deadly.has(AscensionLevel::None));
    assert!(deadly.has(AscensionLevel::Poverty));
    assert!(deadly.has(AscensionLevel::ToughEnemies));
    assert!(deadly.has(deadly));
    assert!(!deadly.has(AscensionLevel::DoubleBoss));

    // Ordering agrees with the ladder, which is what makes `has` a comparison
    // rather than a table.
    assert!(AscensionLevel::Poverty < AscensionLevel::Scarcity);
    assert_eq!(
        AscensionLevel::ALL
            .iter()
            .filter(|l| deadly.has(**l))
            .count(),
        10
    );
}

/// The ladder round-trips, and the wire form is the game's own name.
#[test]
fn a_level_round_trips_through_its_wire_form() {
    for level in AscensionLevel::ALL {
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(
            serde_json::from_str::<AscensionLevel>(&json).unwrap(),
            level
        );
    }

    assert_eq!(wire(&AscensionLevel::AscendersBane), "AscendersBane");
    assert!(serde_json::from_str::<AscensionLevel>("\"NotALevel\"").is_err());
}
