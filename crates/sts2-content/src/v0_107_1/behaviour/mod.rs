//! What this build's content does, as ported code.
//!
//! The generated tables beside this say what the game's data holds. This says
//! what its code does with it. A row is dumped. A rule is read and written out
//! by hand, against the decompiled source.
//!
//! Filed the way `generated/` is, so a thing's behaviour sits under the same
//! name as its row.

pub(crate) mod cards;
pub mod events;
pub(crate) mod monsters;
pub(crate) mod powers;
pub(crate) mod relics;

use sts2_core::{Fight, Played, Taken};

use super::V0_107_1 as V;

/// The characters `ModelDb.AllCharacters` lists, in its order, which is
/// also `UnlockState.CharacterCardPools`' with every character unlocked.
///
/// Not the table's, which is alphabetical. A draw indexes into this one.
pub(crate) const ALL_CHARACTERS: [super::generated::characters::Character; 5] = [
    super::generated::characters::Character::Ironclad,
    super::generated::characters::Character::Silent,
    super::generated::characters::Character::Regent,
    super::generated::characters::Character::Necrobinder,
    super::generated::characters::Character::Defect,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    Card(cards::Resume),
    Potion(crate::behaviour::potions::Resume),
    Monster(monsters::Resume),
}

impl From<crate::behaviour::potions::Resume> for Resume {
    fn from(at: crate::behaviour::potions::Resume) -> Self {
        Resume::Potion(at)
    }
}

pub(crate) fn resume<F: Fight<V>>(fight: &mut F, at: Resume, answer: Taken<V>) -> Played<V> {
    match (at, answer) {
        (Resume::Card(at), Taken::Selected(cards)) => cards::resume_play(fight, at, cards),
        (Resume::Card(at), Taken::Chosen(card)) => cards::resume_chosen(fight, at, card),
        (Resume::Card(at), Taken::Waited) => cards::resume_waited(fight, at),
        (Resume::Card(at), Taken::Drew(drew)) => cards::resume_drew(fight, at, &drew),
        (Resume::Potion(at), answer) => crate::behaviour::potions::resume(fight, at, answer),
        (Resume::Monster(at), Taken::Chosen(card)) => monsters::resume(fight, at, card),
        (at, answer) => unimplemented!("{at:?} resumed with {answer:?}"),
    }
}
