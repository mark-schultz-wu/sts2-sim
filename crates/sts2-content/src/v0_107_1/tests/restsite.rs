//! The nine rest-site options against the dump they came from.
//!
//! The shortest file here, because the domain is the shortest: a closed set of
//! names and nothing else. What that leaves worth testing is the two things a
//! replay alone misses. That the name on the wire is the game's `OptionId`
//! exactly, and that the set has not changed under a dump nobody re-read.

use super::super::restsite::*;
use super::super::{RESTSITE, VERSION};
use super::{build, rows, wire};

/// The anchor: a dump from another build has another hash.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(RESTSITE),
        VERSION,
        "restsite.txt was dumped from a different build than this module claims"
    );
}

/// `RestSiteOption` is the dump's options, once each and in its order.
#[test]
fn the_generated_options_are_the_dumps_options() {
    let dumped: Vec<&str> = rows(RESTSITE, "option").map(|r| r[1]).collect();
    let generated: Vec<String> = RestSiteOption::ALL.iter().map(wire).collect();

    assert_eq!(
        generated, dumped,
        "RestSiteOption is not the dump's options"
    );
}

/// The wire spelling is `OptionId`, which is what an observation carries.
///
/// The mod writes `o.OptionId` straight into the snapshot, so the rename is the
/// whole of the mapping. A wrong one reads as an option this build does not
/// have. Parsed as well as written, because that is the direction the
/// observation goes.
#[test]
fn an_option_parses_from_the_name_the_game_sends() {
    assert_eq!(
        serde_json::from_str::<RestSiteOption>("\"SMITH\"").unwrap(),
        RestSiteOption::Smith
    );
    assert_eq!(
        serde_json::from_str::<RestSiteOption>("\"HEAL\"").unwrap(),
        RestSiteOption::Heal
    );
    assert!(serde_json::from_str::<RestSiteOption>("\"REST\"").is_err());
    assert!(serde_json::from_str::<RestSiteOption>("\"Smith\"").is_err());
}

/// The nine, pinned by name.
///
/// The replay cannot notice a build that adds or drops an option, because the
/// dump and the table move together. Only a literal notices, and a compile-time
/// array makes the diff a line a person has to write.
///
/// Worth pinning rather than counting. A rest site offers `HEAL` and `SMITH`
/// until a relic adds one, so seven of the nine are a relic's effect.
#[test]
fn the_options_this_build_offers() {
    assert_eq!(
        RestSiteOption::ALL.map(|o| wire(&o)),
        [
            "CLONE", "COOK", "DIG", "HATCH", "HEAL", "KINDLE", "LIFT", "MEND", "SMITH"
        ],
        "the things a rest site can offer have changed"
    );
}
