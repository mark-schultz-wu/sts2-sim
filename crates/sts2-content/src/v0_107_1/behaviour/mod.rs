//! What this build's content does, as ported code.
//!
//! The generated tables beside this say what the game's data holds. This says
//! what its code does with it. A row is dumped. A rule is read and written out
//! by hand, against the decompiled source.
//!
//! Filed the way `generated/` is, so a thing's behaviour sits under the same
//! name as its row.

pub(crate) mod acts;
pub(crate) mod cards;
pub(crate) mod encounters;
pub mod events;
pub(crate) mod monsters;
pub(crate) mod potions;
pub(crate) mod powers;
pub(crate) mod relics;

use sts2_core::{Fight, Played, Taken};

use super::V0_107_1 as V;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    Card(cards::Resume),
    Potion(potions::Resume),
}

pub(crate) fn resume<F: Fight<V>>(fight: &mut F, at: Resume, answer: Taken<V>) -> Played<V> {
    match (at, answer) {
        (Resume::Card(at), Taken::Selected(cards)) => cards::resume_play(fight, at, cards),
        (Resume::Potion(at), answer) => potions::resume(fight, at, answer),
        (at, answer) => unimplemented!("{at:?} resumed with {answer:?}"),
    }
}
