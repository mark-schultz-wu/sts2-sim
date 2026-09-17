//! `EncounterModel.GenerateMonsters`, ported.
//!
//! The roster an encounter rolls, for the encounters that roll one. Most field
//! a fixed list, and that is a row rather than code.
//!
//! One function matching on the encounter, the way `EncounterModel`'s
//! subclasses override it. An encounter nobody has ported panics naming
//! itself.

use sts2_core::Spawned;
use sts2_core::rng::{Bound, GameRng, RngInteger};

use super::super::V0_107_1 as V;
use super::super::generated::encounters::Encounter;
use super::super::generated::monsters::{Monster, MoveId};

/// The creatures an encounter rolls, drawn from its own stream.
///
/// # Panics
///
/// An encounter whose generator nobody has ported.
pub(crate) fn generate<R: GameRng>(encounter: Encounter, rng: &mut R) -> Vec<Spawned<V>> {
    match encounter {
        Encounter::SlimesWeak => slimes_weak(rng),
        Encounter::SlimesNormal => slimes_normal(rng),
        Encounter::DecimillipedeElite => decimillipede_elite(rng),
        encounter => unimplemented!("the roster {encounter:?} rolls"),
    }
}

/// `DecimillipedeElite`. Three segments, each opening on its own move.
///
/// One draw picks the front segment's, and the other two follow it round the
/// three the graph cycles through. So the row always advertises all three at
/// once, and which segment has which is what the draw decides.
fn decimillipede_elite<R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    // `StarterMoveIdx % 3`, as `GenerateMoveStateMachine` switches on it.
    const CYCLE: [MoveId; 3] = [MoveId::WritheMove, MoveId::BulkMove, MoveId::ConstrictMove];

    let first: usize =
        num_traits::cast(rng.next_int(Bound::try_new(three::<R>()).expect("three starting moves")))
            .expect("a three-way draw is a valid cycle index");
    [
        Monster::DecimillipedeSegmentFront,
        Monster::DecimillipedeSegmentMiddle,
        Monster::DecimillipedeSegmentBack,
    ]
    .into_iter()
    .enumerate()
    .map(|(at, monster)| Spawned::opening_on(monster, CYCLE[(first + at) % CYCLE.len()]))
    .collect()
}

/// The RNG integer three, which has no literal in the trait's own arithmetic.
fn three<R: GameRng>() -> R::Int {
    R::Int::ONE + R::Int::ONE + R::Int::ONE
}

/// `SlimesWeak`. A small slime, a medium one, then the other small one.
///
/// `Rng.NextItem` over a list the first draw is removed from, so the second
/// draw is over one item and takes it.
fn slimes_weak<R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let small = [Monster::LeafSlimeS, Monster::TwigSlimeS];
    let first: usize = num_traits::cast(
        rng.next_int(Bound::try_new(R::Int::ONE + R::Int::ONE).expect("two small slimes")),
    )
    .expect("a two-way draw is zero or one");
    rng.next_int(Bound::try_new(R::Int::ONE).expect("one small slime remains"));
    let medium_at: usize = num_traits::cast(
        rng.next_int(Bound::try_new(R::Int::ONE + R::Int::ONE).expect("two medium slimes")),
    )
    .expect("a two-way draw is zero or one");
    let medium = [Monster::LeafSlimeM, Monster::TwigSlimeM][medium_at];
    [small[first], medium, small[1 - first]]
        .into_iter()
        .map(Spawned::new)
        .collect()
}

/// `SlimesNormal`. Both medium slimes, then both small ones.
///
/// One `NextBool` decides which small slime stands in front of the other.
/// Nothing else about the roster is rolled: the two medium slimes are always
/// there, and always in that order.
fn slimes_normal<R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let (third, fourth) = match rng.next_bool() {
        true => (Monster::LeafSlimeS, Monster::TwigSlimeS),
        false => (Monster::TwigSlimeS, Monster::LeafSlimeS),
    };
    [Monster::TwigSlimeM, Monster::LeafSlimeM, third, fourth]
        .into_iter()
        .map(Spawned::new)
        .collect()
}
