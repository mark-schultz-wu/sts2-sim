//! `EncounterModel.GenerateMonsters`, ported.
//!
//! The roster an encounter rolls, for the encounters that roll one. Most field
//! a fixed list, and that is a row rather than code.
//!
//! One function matching on the encounter, the way `EncounterModel`'s
//! subclasses override it. An encounter nobody has ported panics naming
//! itself.

use sts2_core::rng::{Bound, GameRng};

use super::super::generated::encounters::Encounter;
use super::super::generated::monsters::Monster;

/// The creatures an encounter rolls, drawn from its own stream.
///
/// # Panics
///
/// An encounter whose generator nobody has ported.
pub(crate) fn generate<R: GameRng>(encounter: Encounter, rng: &mut R) -> Vec<Monster> {
    match encounter {
        Encounter::SlimesWeak => slimes_weak(rng),
        Encounter::SlimesNormal => slimes_normal(rng),
        encounter => unimplemented!("the roster {encounter:?} rolls"),
    }
}

/// `SlimesWeak`. A small slime, a medium one, then the other small one.
///
/// `Rng.NextItem` over a list the first draw is removed from, so the second
/// draw is over one item and takes it.
fn slimes_weak<R: GameRng>(rng: &mut R) -> Vec<Monster> {
    let small = [Monster::LeafSlimeS, Monster::TwigSlimeS];
    let first = rng.next_int(Bound::new(2).expect("two small slimes")) as usize;
    let second = rng.next_int(Bound::new(1).expect("one small slime remains"));
    debug_assert_eq!(second, 0);
    let medium = [Monster::LeafSlimeM, Monster::TwigSlimeM]
        [rng.next_int(Bound::new(2).expect("two medium slimes")) as usize];
    vec![small[first], medium, small[1 - first]]
}

/// `SlimesNormal`. Both medium slimes, then both small ones.
///
/// One `NextBool` decides which small slime stands in front of the other.
/// Nothing else about the roster is rolled: the two medium slimes are always
/// there, and always in that order.
fn slimes_normal<R: GameRng>(rng: &mut R) -> Vec<Monster> {
    let (third, fourth) = match rng.next_bool() {
        true => (Monster::LeafSlimeS, Monster::TwigSlimeS),
        false => (Monster::TwigSlimeS, Monster::LeafSlimeS),
    };
    vec![Monster::TwigSlimeM, Monster::LeafSlimeM, third, fourth]
}
