//! The character table against the dump it came from.
//! The smallest domain, and the one that points at the others. Most of what a character is, is a reference into a table another dump printed. Therefore most of what is worth checking is that those references land where the dump said, and that the compiler is doing the rest.

use super::super::cards::Cards;
use super::super::characters::*;
use super::super::relics::RelicPool;
use super::super::{CHARACTERS, VERSION};
use super::{build, rows, wire};
use std::collections::BTreeMap;

fn character(id: &str) -> Character {
    *Character::ALL
        .iter()
        .find(|c| wire(c) == id)
        .unwrap_or_else(|| panic!("{id} is in the dump but not generated"))
}

/// The anchor: a dump from another build has another hash.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(CHARACTERS),
        VERSION,
        "characters.txt was dumped from a different build than this module claims"
    );
}

#[test]
fn the_generated_table_matches_the_dump() {
    let mut checked = 0;

    for row in rows(CHARACTERS, "character") {
        let id = row[1];
        let d = character(id).data();

        assert_eq!(d.playable, row[2] == "1", "{id}: playable");
        assert_eq!(wire(&d.gender), row[3], "{id}: gender");
        assert_eq!(d.starting_hp.to_string(), row[4], "{id}: starting hp");
        assert_eq!(d.starting_gold.to_string(), row[5], "{id}: starting gold");
        assert_eq!(d.max_energy.to_string(), row[6], "{id}: max energy");
        assert_eq!(d.orb_slots.to_string(), row[7], "{id}: orb slots");
        assert_eq!(wire(&d.card_pool), row[9], "{id}: card pool");
        assert_eq!(wire(&d.relic_pool), row[10], "{id}: relic pool");
        assert_eq!(wire(&d.potion_pool), row[11], "{id}: potion pool");

        let unlocks = d.unlocks_after.map_or("-".to_string(), |c| wire(&c));
        assert_eq!(unlocks, row[12], "{id}: unlocks after");

        checked += 1;
    }

    assert_eq!(checked, 8, "the build has 8 characters");
    assert_eq!(Character::ALL.len(), 8);
}

/// The starting deck, in order and with the right card from the right pool.
///
/// Cards are pool-scoped, `Strike` is a variant of five different enums, so the deck is checked against both columns the dump writes. Getting only the id right will put the Ironclad's Strike in the Silent's deck and nothing will notice.
#[test]
fn the_starting_deck_is_the_dumped_one_in_order() {
    let mut dumped: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for row in rows(CHARACTERS, "deck") {
        dumped.entry(row[1]).or_default().push((row[3], row[4]));
    }

    for c in Character::ALL {
        let id = wire(&c);
        let want = dumped.get(id.as_str()).cloned().unwrap_or_default();
        let got: Vec<Cards> = c.data().deck.iter().collect();

        assert_eq!(got.len(), want.len(), "{id}: deck size");
        for (card, (dumped_id, _)) in got.iter().zip(&want) {
            assert_eq!(&wire(card), dumped_id, "{id}: a card in the deck");
        }
    }

    // The Silent opens with twelve cards where everyone else opens with ten,
    // and two characters open with none.
    assert_eq!(character("SILENT").data().deck.len(), 12);
    assert_eq!(character("IRONCLAD").data().deck.len(), 10);
    assert_eq!(character("DEPRIVED").data().deck.len(), 0);
    assert_eq!(character("DEPRECATED_CHARACTER").data().deck.len(), 0);
}

/// A starting deck is not always drawn from the character's own card pool.
///
/// This is the reason the dump writes the pool beside each card rather than letting codegen infer it. `RANDOM_CHARACTER` opens with a Strike and a Defend from each of the five character pools while its own `card_pool` is the Ironclad's. Therefore inferring has named the wrong variant for eight of its ten cards, and every one of them will still have compiled.
#[test]
fn a_starting_deck_can_come_from_more_than_one_pool() {
    let mut foreign: Vec<&str> = Vec::new();
    let mut own: BTreeMap<&str, &str> = BTreeMap::new();
    for row in rows(CHARACTERS, "character") {
        own.insert(row[1], row[9]);
    }

    for row in rows(CHARACTERS, "deck") {
        if own[row[1]] != row[4] && !foreign.contains(&row[1]) {
            foreign.push(row[1]);
        }
    }

    assert_eq!(
        foreign,
        ["RANDOM_CHARACTER"],
        "the characters starting with a card from another pool have changed"
    );

    // Its deck really does span the five, which is what makes it the test case.
    let pools: std::collections::BTreeSet<&str> = rows(CHARACTERS, "deck")
        .filter(|r| r[1] == "RANDOM_CHARACTER")
        .map(|r| r[4])
        .collect();
    assert_eq!(pools.len(), 5);
}

/// One starting relic each, or none.
#[test]
fn the_starting_relics_are_the_dumped_ones() {
    let mut dumped: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for row in rows(CHARACTERS, "startingrelic") {
        dumped.entry(row[1]).or_default().push(row[3]);
    }

    for c in Character::ALL {
        let id = wire(&c);
        let want = dumped.get(id.as_str()).cloned().unwrap_or_default();
        let got: Vec<String> = c.data().starting_relics.iter().map(|r| wire(&r)).collect();
        assert_eq!(got, want, "{id}: starting relics");
        assert!(got.len() <= 1, "{id} starts with more than one relic");
    }

    assert_eq!(
        character("IRONCLAD")
            .data()
            .starting_relics
            .iter()
            .map(|r| wire(&r))
            .collect::<Vec<_>>(),
        ["BURNING_BLOOD"]
    );
}

/// The unlock chain, which is the only edge between characters.
///
/// A `None` means available from the start. Worth pinning as a shape rather than a set of pairs: it is a chain. Therefore a build that made it a tree or a cycle is saying something new about how a run is offered.
#[test]
fn the_unlock_chain_is_a_chain() {
    let mut unlocked_by: BTreeMap<String, String> = BTreeMap::new();
    for c in Character::ALL {
        if let Some(after) = c.data().unlocks_after {
            unlocked_by.insert(wire(&c), wire(&after));
        }
    }

    assert_eq!(
        unlocked_by,
        BTreeMap::from([
            ("DEFECT".to_string(), "NECROBINDER".to_string()),
            ("NECROBINDER".to_string(), "REGENT".to_string()),
            ("REGENT".to_string(), "SILENT".to_string()),
        ]),
        "the unlock chain has changed"
    );

    // No character unlocks two others, which is what makes it a chain.
    let mut targets: Vec<&String> = unlocked_by.values().collect();
    let before = targets.len();
    targets.sort_unstable();
    targets.dedup();
    assert_eq!(
        targets.len(),
        before,
        "two characters unlock after the same run"
    );

    // Ironclad and Silent are the two available from the start.
    let free: Vec<String> = Character::ALL
        .iter()
        .filter(|c| c.data().playable && c.data().unlocks_after.is_none())
        .map(wire)
        .collect();
    assert_eq!(free, ["IRONCLAD", "SILENT"]);
}

/// Only the Defect starts with orb slots, and only the playable five matter.
#[test]
fn the_numbers_are_the_ones_a_run_starts_with() {
    let orbs: Vec<(String, u8)> = Character::ALL
        .iter()
        .filter(|c| c.data().orb_slots > 0)
        .map(|c| (wire(c), c.data().orb_slots))
        .collect();
    assert_eq!(orbs, [("DEFECT".to_string(), 3)]);

    // Every playable character starts on three energy and 99 gold. The two
    // placeholders are the ones with 100 energy and 1000 HP, which is how you
    // can tell them apart from content.
    for c in Character::ALL.iter().filter(|c| c.data().playable) {
        let d = c.data();
        assert_eq!(d.max_energy, 3, "{}: max energy", wire(c));
        assert_eq!(d.starting_gold, 99, "{}: starting gold", wire(c));
        assert!(
            (66..=80).contains(&d.starting_hp),
            "{}: starting hp {}",
            wire(c),
            d.starting_hp
        );
    }
}

/// Every playable character draws from a pool named after itself.
///
/// The three unplayable ones do not, which is the point. `RANDOM_CHARACTER` borrows the Ironclad's pools and the two placeholders draw cards from `MOCK_CARD_POOL`, a pool the cards domain does not generate at all. The reason `CardPool` is declared in this domain rather than pointed at.
#[test]
fn a_playable_character_draws_from_its_own_pools() {
    for c in Character::ALL.iter().filter(|c| c.data().playable) {
        let (id, d) = (wire(c), c.data());
        assert_eq!(wire(&d.card_pool), format!("{id}_CARD_POOL"), "{id}: cards");
        assert_eq!(
            wire(&d.relic_pool),
            format!("{id}_RELIC_POOL"),
            "{id}: relics"
        );
        assert_eq!(
            wire(&d.potion_pool),
            format!("{id}_POTION_POOL"),
            "{id}: potions"
        );
    }

    assert_eq!(
        wire(&character("DEPRIVED").data().card_pool),
        "MOCK_CARD_POOL"
    );
    assert_eq!(
        character("RANDOM_CHARACTER").data().relic_pool,
        RelicPool::Ironclad
    );
}

/// The wire form round-trips, and it is the only spelling of an id.
#[test]
fn a_character_round_trips_through_its_wire_form() {
    let dumped: Vec<&str> = rows(CHARACTERS, "character").map(|r| r[1]).collect();

    for (c, id) in Character::ALL.iter().zip(&dumped) {
        let json = serde_json::to_string(c).unwrap();
        assert_eq!(json, format!("\"{id}\""), "the rename is not the dump's id");
        assert_eq!(&serde_json::from_str::<Character>(&json).unwrap(), c);
    }

    assert!(serde_json::from_str::<Character>("\"NOT_A_CHARACTER\"").is_err());
}

/// Starting HP and gold do not move with ascension.
///
/// The dump read both at all eleven levels and emitted nothing, which is worth a test because the first game did scale starting HP. Therefore this is a fact about this build rather than an assumption carried over.
#[test]
fn no_character_starts_differently_at_a_higher_ascension() {
    assert_eq!(rows(CHARACTERS, "scalestart").count(), 0);
}
