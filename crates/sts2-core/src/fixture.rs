//! Constants a test writes, for the types that refuse the number `255`.
//!
//! [`CombatId`], [`HandIndex`] and [`BeltIndex`] each refuse the largest byte.
//! So a test that spells one from a literal has a `255` to answer for. It
//! answers nothing: the test wrote the number. These panic, in one place rather
//! than in every test that spells an id.
//!
//! Behind the `fixture` feature, which only a `dev-dependencies` entry turns
//! on. No shipped code sees this module.

use crate::{BeltIndex, CombatId, HandIndex};

/// The creature this number names.
///
/// # Panics
///
/// If the number is `255`, which names no creature.
pub fn at(id: u8) -> CombatId {
    CombatId::try_new(id).expect("a test spells a creature the fight has")
}

/// The place in the hand this number names.
///
/// # Panics
///
/// If the number is `255`, which names no card.
pub fn hand(index: u8) -> HandIndex {
    HandIndex::try_new(index).expect("a test spells a card the hand holds")
}

/// The slot in the belt this number names.
///
/// # Panics
///
/// If the number is `255`, which names no slot.
pub fn belt(index: u8) -> BeltIndex {
    BeltIndex::try_new(index).expect("a test spells a slot the belt has")
}
