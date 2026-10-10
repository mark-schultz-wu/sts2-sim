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
        Encounter::RubyRaidersNormal => ruby_raiders_normal(rng),
        Encounter::SlitheringStranglerNormal => slithering_strangler_normal(rng),
        Encounter::FlyconidNormal => flyconid_normal(rng),
        Encounter::BowlbugsWeak => bowlbugs_weak(rng),
        Encounter::BowlbugsNormal => bowlbugs_normal(rng),
        Encounter::ScrollsOfBitingWeak => scrolls_of_biting(rng, false),
        Encounter::ScrollsOfBitingNormal => scrolls_of_biting(rng, true),
        Encounter::CorpseSlugsWeak => corpse_slugs_weak(rng),
        Encounter::TwoTailedRatsNormal => two_tailed_rats_normal(rng),
        encounter => unimplemented!("the roster {encounter:?} rolls"),
    }
}

/// `CorpseSlugsWeak`. Two Corpse Slugs, which
/// `EnsureCorpseSlugsStartWithDifferentMoves` sets apart: one draw picks the
/// first's opening, and the second takes the one after it in the cycle.
fn corpse_slugs_weak<R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    // `StarterMoveIdx % 3`, as `GenerateMoveStateMachine` switches on it.
    const CYCLE: [MoveId; 3] = [MoveId::WhipSlapMove, MoveId::GlompMove, MoveId::GoopMove];

    let first: usize =
        num_traits::cast(rng.next_int(Bound::try_new(three::<R>()).expect("three starting moves")))
            .expect("a three-way draw is a valid cycle index");
    (0..2)
        .map(|at| Spawned::opening_on(Monster::CorpseSlug, CYCLE[(first + at) % CYCLE.len()]))
        .collect()
}

/// `TwoTailedRatsNormal`. Three rats in the last three of its five slots, the
/// first two left for those they call. One draw picks the first's opening,
/// and the other two follow it round the cycle.
fn two_tailed_rats_normal<R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    // `StarterMoveIndex % 3`, as `GenerateMoveStateMachine` switches on it.
    const CYCLE: [MoveId; 3] = [
        MoveId::ScratchMove,
        MoveId::DiseaseBiteMove,
        MoveId::ScreechMove,
    ];

    let first: usize =
        num_traits::cast(rng.next_int(Bound::try_new(three::<R>()).expect("three starting moves")))
            .expect("a three-way draw is a valid cycle index");
    (0..3u8)
        .map(|at| Spawned {
            slot: Some(2 + at),
            ..Spawned::opening_on(
                Monster::TwoTailedRat,
                CYCLE[(first + usize::from(at)) % CYCLE.len()],
            )
        })
        .collect()
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

/// `ScrollsOfBitingWeak` and `ScrollsOfBitingNormal`. Three scrolls, each
/// opening on its own move, as the Decimillipede's segments do. The normal
/// encounter adds a fourth, which opens on More Teeth.
fn scrolls_of_biting<R: GameRng>(rng: &mut R, normal: bool) -> Vec<Spawned<V>> {
    // `StarterMoveIdx % 3`, as `GenerateMoveStateMachine` switches on it.
    const CYCLE: [MoveId; 3] = [MoveId::Chomp, MoveId::Chew, MoveId::MoreTeeth];

    let first: usize =
        num_traits::cast(rng.next_int(Bound::try_new(three::<R>()).expect("three starting moves")))
            .expect("a three-way draw is a valid cycle index");
    let mut roster: Vec<_> = (0..CYCLE.len())
        .map(|at| Spawned::opening_on(Monster::ScrollOfBiting, CYCLE[(first + at) % CYCLE.len()]))
        .collect();
    if normal {
        roster.push(Spawned::opening_on(
            Monster::ScrollOfBiting,
            MoveId::MoreTeeth,
        ));
    }
    roster
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

/// `BowlbugsWeak`. The Rock, then one of the Egg and the Nectar by
/// `NextItem`.
fn bowlbugs_weak<R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let bug_at: usize = num_traits::cast(
        rng.next_int(Bound::try_new(R::Int::ONE + R::Int::ONE).expect("two bugs")),
    )
    .expect("a two-way draw is zero or one");
    [
        Monster::BowlbugRock,
        [Monster::BowlbugEgg, Monster::BowlbugNectar][bug_at],
    ]
    .into_iter()
    .map(Spawned::new)
    .collect()
}

/// `BowlbugsNormal`. The Rock, then two workers by `NextItem`, each drawn
/// from those `_workerValidCounts` still allows. Each worker is allowed once,
/// so the second draw is over the two the first left, in the table's order.
fn bowlbugs_normal<R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let mut workers = vec![
        Monster::BowlbugEgg,
        Monster::BowlbugSilk,
        Monster::BowlbugNectar,
    ];
    let mut roster = vec![Monster::BowlbugRock];
    for _ in 0..2 {
        let count: R::Int = num_traits::cast(workers.len()).expect("a few workers");
        let at: usize = num_traits::cast(
            rng.next_int(Bound::try_new(count).expect("a worker is still allowed")),
        )
        .expect("a draw below the count is an index");
        roster.push(workers.remove(at));
    }
    roster.into_iter().map(Spawned::new).collect()
}

/// `FlyconidNormal`. One medium slime by `NextItem`, then the Flyconid.
fn flyconid_normal<R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let medium_at: usize = num_traits::cast(
        rng.next_int(Bound::try_new(R::Int::ONE + R::Int::ONE).expect("two medium slimes")),
    )
    .expect("a two-way draw is zero or one");
    [
        [Monster::LeafSlimeM, Monster::TwigSlimeM][medium_at],
        Monster::Flyconid,
    ]
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

/// `RubyRaidersNormal`. Three raiders, no two the same.
///
/// Each is one `NextItem` over the five, less those already standing, in the
/// order the game's own table lists them.
fn ruby_raiders_normal<R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    const RAIDERS: [Monster; 5] = [
        Monster::AxeRubyRaider,
        Monster::AssassinRubyRaider,
        Monster::BruteRubyRaider,
        Monster::CrossbowRubyRaider,
        Monster::TrackerRubyRaider,
    ];

    let mut standing: Vec<Monster> = Vec::with_capacity(3);
    for _ in 0..3 {
        let left: Vec<Monster> = RAIDERS
            .into_iter()
            .filter(|raider| !standing.contains(raider))
            .collect();
        standing.push(
            *rng.choose(&left)
                .expect("five raiders leave some for a third"),
        );
    }
    standing.into_iter().map(Spawned::new).collect()
}

/// `SlitheringStranglerNormal`. A Snapping Jaxfruit, a medium slime or two
/// small ones, then the Strangler.
///
/// One `NextItem` over the three kinds, in the order the game's enum lists
/// them. The medium slime is one more, and each small slime its own, so the
/// two small ones can be the same.
fn slithering_strangler_normal<R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    const SMALL: [Monster; 2] = [Monster::LeafSlimeS, Monster::TwigSlimeS];
    const MEDIUM: [Monster; 2] = [Monster::LeafSlimeM, Monster::TwigSlimeM];

    let kind: usize = num_traits::cast(
        rng.next_int(Bound::try_new(three::<R>()).expect("three kinds of company")),
    )
    .expect("a three-way draw is a valid kind");
    let mut roster = match kind {
        0 => vec![Monster::SnappingJaxfruit],
        1 => vec![*rng.choose(&MEDIUM).expect("two medium slimes")],
        _ => vec![
            *rng.choose(&SMALL).expect("two small slimes"),
            *rng.choose(&SMALL).expect("two small slimes"),
        ],
    };
    roster.push(Monster::SlitheringStrangler);
    roster.into_iter().map(Spawned::new).collect()
}
