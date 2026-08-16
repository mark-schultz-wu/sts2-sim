//! Content for game build `v0.107.1`.
//!
//! `cards.txt` is dumped from the running game — `STS2_HEADLESS_MODE=carddump`,
//! see `mod/sts2_determinism_test/CardDump.cs`. It carries the content hash in
//! its header, and [`tests`] requires that to match [`VERSION`], so
//! regenerating against a different build fails rather than quietly replacing
//! the table.
//!
//! Static means true of the card with no run in progress. `Type` and
//! `TargetType` are virtual, and a few cards compute them from combat state —
//! those cells read `dynamic`, and the simulator has to ask rather than look
//! up. Detection is conservative: a constant override is marked too, because
//! marking a constant as dynamic costs one method and marking a dynamic as
//! constant costs a divergence nobody can see.

use crate::Version;

pub const VERSION: Version = Version {
    name: "v0.107.1",
    content_hash: 3_954_186_980,
};

/// The dump, verbatim. Parsed by the tests; the generated tables are built from
/// it by `cargo xtask codegen`.
pub const CARDS: &str = include_str!("cards.txt");

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> impl Iterator<Item = Vec<&'static str>> {
        CARDS
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| l.split_whitespace().collect())
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
        let cards: Vec<_> = rows().collect();
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

        for row in rows() {
            assert!(
                POOLS.contains(&row[2]),
                "unknown pool for {}: {}",
                row[1],
                row[2]
            );
        }
    }

    /// Ids are the enum variants and the wire form, so a duplicate would make
    /// two different cards the same card.
    #[test]
    fn card_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for row in rows() {
            assert!(seen.insert(row[1]), "duplicate card id {}", row[1]);
        }
    }

    /// Exactly the four cards that compute a field, and no others. If a future
    /// build adds one, this fails rather than the simulator silently reading a
    /// constant that is not one.
    #[test]
    fn only_the_known_cards_are_dynamic() {
        let dynamic: Vec<&str> = rows()
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
