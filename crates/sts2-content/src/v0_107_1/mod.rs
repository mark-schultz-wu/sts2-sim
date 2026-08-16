//! Content for game build `v0.107.1`.
//!
//! `dumps/` holds what the running game printed — `STS2_HEADLESS_MODE=carddump`
//! and friends, see `mod/sts2_determinism_test/`. Each dump carries the content
//! hash in its header, and [`tests`] requires that to match [`VERSION`], so
//! regenerating against a different build fails rather than quietly replacing
//! the tables. `generated/` mirrors it, one module per kind of content.
//!
//! Static means true of the card with no run in progress. `Type` and
//! `TargetType` are virtual, and a few cards compute them from combat state —
//! those cells read `dynamic`, and the simulator has to ask rather than look
//! up. Detection is conservative: a constant override is marked too, because
//! marking a constant as dynamic costs one method and marking a dynamic as
//! constant costs a divergence nobody can see.

mod generated;

pub use generated::*;

use crate::Version;

pub const VERSION: Version = Version {
    name: "v0.107.1",
    content_hash: 3_954_186_980,
};

/// The card dump, verbatim — test data, not runtime data.
///
/// `cargo xtask codegen` reads it from disk to build [`cards`], and the tests
/// below replay it to check that table. Nothing at runtime should parse it:
/// that is what the generated table is for, and a `pub const` here would both
/// invite it and put 38KB of text into every binary that links this crate.
#[cfg(test)]
const CARDS: &str = include_str!("dumps/cards.txt");

#[cfg(test)]
mod tests {
    use super::cards::*;
    use super::*;
    use crate::{Cost, Field};

    /// The dump's `cost` and `costsX` columns as the `Cost` they should have
    /// generated, spelled out independently of the codegen that produced the
    /// table — otherwise the cross-check would only prove codegen agrees with
    /// itself.
    ///
    /// The two columns are not independent. `CardEnergyCost` sets
    /// `Canonical = CostsX ? 0 : canonicalCost`, so an X-cost card carries no
    /// number, and it treats any negative base as "no cost", so `-1` is a
    /// sentinel rather than a cost of minus one. Both are asserted here: if a
    /// future build breaks either, this fails instead of silently encoding an
    /// X-cost card as costing whatever number came along.
    fn cost(canonical: &str, costs_x: &str) -> Cost {
        match (costs_x, canonical) {
            ("1", n) => {
                assert_eq!(n, "0", "an X-cost card carries a canonical cost");
                Cost::X
            }
            (_, "-1") => Cost::None,
            (_, n) => Cost::Energy(n.parse().expect("a non-negative energy cost")),
        }
    }

    /// The game's own spelling of a value, which is the serde rename rather
    /// than the Rust identifier.
    fn wire<T: serde::Serialize>(value: &T) -> String {
        serde_json::to_string(value)
            .unwrap()
            .trim_matches('"')
            .to_string()
    }

    /// The generated table has to agree with the dump it was generated from —
    /// otherwise `cargo xtask codegen` was not re-run and the table describes a
    /// build nobody is talking to.
    #[test]
    fn the_generated_table_matches_the_dump() {
        let mut checked = 0;
        for row in rows("card") {
            let card = Cards::ALL
                .iter()
                .find(|c| c.id() == row[1])
                .unwrap_or_else(|| panic!("{} is in the dump but not generated", row[1]));

            let d = card.data();
            assert_eq!(d.cost, cost(row[6], row[7]), "{}: cost", row[1]);
            assert_eq!(d.max_upgrade.to_string(), row[8], "{}: max_upgrade", row[1]);
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
            let expected = if row[9] == "-" {
                Vec::new()
            } else {
                row[9].split(',').map(String::from).collect()
            };
            assert_eq!(keywords, expected, "{}: keywords", row[1]);

            checked += 1;
        }
        assert_eq!(checked, 578);
    }

    fn rows(kind: &'static str) -> impl Iterator<Item = Vec<&'static str>> {
        CARDS
            .lines()
            .map(|l| l.split_whitespace().collect::<Vec<_>>())
            .filter(move |f| f.first() == Some(&kind))
    }

    /// The anchor. A dump from another build has another hash, and a table
    /// silently regenerated against one is the failure this pairing exists to
    /// prevent.
    #[test]
    fn the_dump_came_from_this_version() {
        let header = CARDS.lines().next().expect("the dump has a header");
        let hash: u32 = header
            .rsplit_once(' ')
            .expect("the header names a content hash")
            .1
            .parse()
            .expect("the content hash is a number");

        assert_eq!(
            hash, VERSION.content_hash,
            "cards.txt was dumped from a different build than this module claims"
        );
    }

    #[test]
    fn every_card_is_present_and_well_formed() {
        let cards: Vec<_> = rows("card").collect();
        assert_eq!(cards.len(), 578, "the build has 578 cards");

        for row in &cards {
            assert_eq!(row.len(), 10, "a card row has ten fields: {row:?}");
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

        for row in rows("card") {
            assert!(
                POOLS.contains(&row[2]),
                "unknown pool for {}: {}",
                row[1],
                row[2]
            );
        }
    }

    /// The enums the card fields draw from, reflected from the game rather
    /// than inferred from the values in use — a member no card happens to use
    /// is still a member, and an enum missing one fails to decode the first
    /// time content grows into it.
    #[test]
    fn the_field_enums_are_complete() {
        for (name, count) in [
            ("CardType", 7),
            ("CardRarity", 11),
            ("TargetType", 10),
            ("CardKeyword", 8),
        ] {
            let members: Vec<_> = rows("enum").filter(|r| r[1] == name).collect();
            assert_eq!(members.len(), count, "{name} member count changed");

            // Discriminants are the wire and table encoding, so a gap or a
            // reorder is a silent remapping rather than an addition.
            for (i, m) in members.iter().enumerate() {
                assert_eq!(m[2], i.to_string(), "{name} is no longer densely numbered");
            }
        }
    }

    /// Three members the game declares are deliberately not generated: they
    /// are the C# zero rather than values of the field they name, and each is
    /// an `Option` at the one place that needs it. See `DROPPED` in
    /// `xtask/src/codegen.rs` for the evidence per member.
    ///
    /// Dropping one is only sound while no card carries it, so that is what
    /// this pins. The converse is not asserted — a generated member no card
    /// uses is normal, which is why `TargetType`'s unused `AnyPlayer`,
    /// `TargetedNoCreature` and `Osty` are still generated.
    #[test]
    fn no_card_uses_a_member_that_is_not_generated() {
        for (column, member) in [(3, "CardType::None"), (4, "CardRarity::None")] {
            let uses: Vec<&str> = rows("card")
                .filter(|r| r[column] == "None")
                .map(|r| r[1])
                .collect();
            assert!(uses.is_empty(), "{member} is on {uses:?}");
        }

        let keywords: Vec<&str> = rows("card")
            .filter(|r| r[9].split(',').any(|k| k == "None"))
            .map(|r| r[1])
            .collect();
        assert!(keywords.is_empty(), "CardKeyword::None is on {keywords:?}");
    }

    /// Ids are the enum variants and the wire form, so a duplicate would make
    /// two different cards the same card.
    #[test]
    fn card_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for row in rows("card") {
            assert!(seen.insert(row[1]), "duplicate card id {}", row[1]);
        }
    }

    /// `max_upgrade` is a `u8`, so a build that grew a negative or a large one
    /// would truncate silently. It is 0 or 1 on every shipped card — but
    /// `CardModel` has a real `if (MaxUpgradeLevel > 1)` branch, so the field
    /// is a count rather than a flag and this is the check that it stays small.
    #[test]
    fn upgrade_levels_stay_in_a_byte() {
        let mut seen: Vec<&str> = rows("card").map(|r| r[8]).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen, ["0", "1"], "max upgrade levels changed");
    }

    /// Exactly the four cards that compute a field, and no others. If a future
    /// build adds one, this fails rather than the simulator silently reading a
    /// constant that is not one.
    #[test]
    fn only_the_known_cards_are_dynamic() {
        let dynamic: Vec<&str> = rows("card")
            .filter(|r| r[3] == "dynamic" || r[5] == "dynamic")
            .map(|r| r[1])
            .collect();

        assert_eq!(
            dynamic,
            ["MAD_SCIENCE", "MALAISE", "SHIV", "SOVEREIGN_BLADE"],
            "the set of cards computing Type or TargetType has changed"
        );
    }
}
