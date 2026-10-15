//! `EncounterModel.GenerateMonsters`, ported.
//!
//! The roster an encounter rolls, for the encounters that roll one. Most field
//! a fixed list, and that is a row rather than code.
//!
//! One function matching on the encounter, the way `EncounterModel`'s
//! subclasses override it. An encounter nobody has ported panics naming
//! itself.

use crate::ids::{EncounterIds as _, MonsterIds as _, MoveIds as _};
use sts2_core::Spawned;
use sts2_core::rng::{Bound, GameRng, RngInteger, Span};

/// The creatures an encounter rolls, drawn from its own stream.
///
/// # Panics
///
/// An encounter whose generator nobody has ported.
pub(crate) fn generate<V: crate::Build, R: GameRng>(
    encounter: V::Encounter,
    rng: &mut R,
) -> Vec<Spawned<V>> {
    match encounter {
        __c0 if __c0 == V::Encounter::SLIMES_WEAK => slimes_weak::<V, _>(rng),
        __c0 if __c0 == V::Encounter::SLIMES_NORMAL => slimes_normal::<V, _>(rng),
        __c0 if __c0 == V::Encounter::DECIMILLIPEDE_ELITE => decimillipede_elite::<V, _>(rng),
        __c0 if __c0 == V::Encounter::RUBY_RAIDERS_NORMAL => ruby_raiders_normal::<V, _>(rng),
        __c0 if __c0 == V::Encounter::SLITHERING_STRANGLER_NORMAL => {
            slithering_strangler_normal::<V, _>(rng)
        }
        __c0 if __c0 == V::Encounter::FLYCONID_NORMAL => flyconid_normal::<V, _>(rng),
        __c0 if __c0 == V::Encounter::BOWLBUGS_WEAK => bowlbugs_weak::<V, _>(rng),
        __c0 if __c0 == V::Encounter::BOWLBUGS_NORMAL => bowlbugs_normal::<V, _>(rng),
        __c0 if __c0 == V::Encounter::SCROLLS_OF_BITING_WEAK => {
            scrolls_of_biting::<V, _>(rng, false)
        }
        __c0 if __c0 == V::Encounter::SCROLLS_OF_BITING_NORMAL => {
            scrolls_of_biting::<V, _>(rng, true)
        }
        __c0 if __c0 == V::Encounter::CORPSE_SLUGS_WEAK => corpse_slugs::<V, _>(rng, 2),
        __c0 if __c0 == V::Encounter::CORPSE_SLUGS_NORMAL => corpse_slugs::<V, _>(rng, 3),
        __c0 if __c0 == V::Encounter::TWO_TAILED_RATS_NORMAL => two_tailed_rats_normal::<V, _>(rng),
        __c0 if __c0 == V::Encounter::PUNCH_OFF_EVENT_ENCOUNTER => punch_off::<V, _>(rng),
        encounter => unimplemented!("the roster {encounter:?} rolls"),
    }
}

/// `CorpseSlugsWeak` and `CorpseSlugsNormal`, two Corpse Slugs and three.
/// `EnsureCorpseSlugsStartWithDifferentMoves` sets them apart: one draw picks
/// the first's opening, and each after it takes the next in the cycle.
fn corpse_slugs<V: crate::Build, R: GameRng>(rng: &mut R, slugs: usize) -> Vec<Spawned<V>> {
    // `StarterMoveIdx % 3`, as `GenerateMoveStateMachine` switches on it.
    let cycle: [V::MoveId; 3] = [
        V::MoveId::WHIP_SLAP_MOVE,
        V::MoveId::GLOMP_MOVE,
        V::MoveId::GOOP_MOVE,
    ];

    let first: usize =
        num_traits::cast(rng.next_int(Bound::try_new(three::<R>()).expect("three starting moves")))
            .expect("a three-way draw is a valid cycle index");
    (0..slugs)
        .map(|at| Spawned::opening_on(V::Monster::CORPSE_SLUG, cycle[(first + at) % cycle.len()]))
        .collect()
}

/// `TwoTailedRatsNormal`. Three rats in the last three of its five slots, the
/// first two left for those they call. One draw picks the first's opening,
/// and the other two follow it round the cycle.
fn two_tailed_rats_normal<V: crate::Build, R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    // `StarterMoveIndex % 3`, as `GenerateMoveStateMachine` switches on it.
    let cycle: [V::MoveId; 3] = [
        V::MoveId::SCRATCH_MOVE,
        V::MoveId::DISEASE_BITE_MOVE,
        V::MoveId::SCREECH_MOVE,
    ];

    let first: usize =
        num_traits::cast(rng.next_int(Bound::try_new(three::<R>()).expect("three starting moves")))
            .expect("a three-way draw is a valid cycle index");
    (0..3u8)
        .map(|at| Spawned {
            slot: Some(2 + at),
            ..Spawned::opening_on(
                V::Monster::TWO_TAILED_RAT,
                cycle[(first + usize::from(at)) % cycle.len()],
            )
        })
        .collect()
}

/// `DecimillipedeElite`. Three segments, each opening on its own move.
///
/// One draw picks the front segment's, and the other two follow it round the
/// three the graph cycles through. So the row always advertises all three at
/// once, and which segment has which is what the draw decides.
fn decimillipede_elite<V: crate::Build, R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    // `StarterMoveIdx % 3`, as `GenerateMoveStateMachine` switches on it.
    let cycle: [V::MoveId; 3] = [
        V::MoveId::WRITHE_MOVE,
        V::MoveId::BULK_MOVE,
        V::MoveId::CONSTRICT_MOVE,
    ];

    let first: usize =
        num_traits::cast(rng.next_int(Bound::try_new(three::<R>()).expect("three starting moves")))
            .expect("a three-way draw is a valid cycle index");
    [
        V::Monster::DECIMILLIPEDE_SEGMENT_FRONT,
        V::Monster::DECIMILLIPEDE_SEGMENT_MIDDLE,
        V::Monster::DECIMILLIPEDE_SEGMENT_BACK,
    ]
    .into_iter()
    .enumerate()
    .map(|(at, monster)| Spawned::opening_on(monster, cycle[(first + at) % cycle.len()]))
    .collect()
}

/// `ScrollsOfBitingWeak` and `ScrollsOfBitingNormal`. Three scrolls, each
/// opening on its own move, as the Decimillipede's segments do. The normal
/// encounter adds a fourth, which opens on More Teeth.
fn scrolls_of_biting<V: crate::Build, R: GameRng>(rng: &mut R, normal: bool) -> Vec<Spawned<V>> {
    // `StarterMoveIdx % 3`, as `GenerateMoveStateMachine` switches on it.
    let cycle: [V::MoveId; 3] = [V::MoveId::CHOMP, V::MoveId::CHEW, V::MoveId::MORE_TEETH];

    let first: usize =
        num_traits::cast(rng.next_int(Bound::try_new(three::<R>()).expect("three starting moves")))
            .expect("a three-way draw is a valid cycle index");
    let mut roster: Vec<_> = (0..cycle.len())
        .map(|at| {
            Spawned::opening_on(
                V::Monster::SCROLL_OF_BITING,
                cycle[(first + at) % cycle.len()],
            )
        })
        .collect();
    if normal {
        roster.push(Spawned::opening_on(
            V::Monster::SCROLL_OF_BITING,
            V::MoveId::MORE_TEETH,
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
fn slimes_weak<V: crate::Build, R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let small = [V::Monster::LEAF_SLIME_S, V::Monster::TWIG_SLIME_S];
    let first: usize = num_traits::cast(
        rng.next_int(Bound::try_new(R::Int::ONE + R::Int::ONE).expect("two small slimes")),
    )
    .expect("a two-way draw is zero or one");
    rng.next_int(Bound::try_new(R::Int::ONE).expect("one small slime remains"));
    let medium_at: usize = num_traits::cast(
        rng.next_int(Bound::try_new(R::Int::ONE + R::Int::ONE).expect("two medium slimes")),
    )
    .expect("a two-way draw is zero or one");
    let medium = [V::Monster::LEAF_SLIME_M, V::Monster::TWIG_SLIME_M][medium_at];
    [small[first], medium, small[1 - first]]
        .into_iter()
        .map(Spawned::new)
        .collect()
}

/// `BowlbugsWeak`. The Rock, then one of the Egg and the Nectar by
/// `NextItem`.
fn bowlbugs_weak<V: crate::Build, R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let bug_at: usize = num_traits::cast(
        rng.next_int(Bound::try_new(R::Int::ONE + R::Int::ONE).expect("two bugs")),
    )
    .expect("a two-way draw is zero or one");
    [
        V::Monster::BOWLBUG_ROCK,
        [V::Monster::BOWLBUG_EGG, V::Monster::BOWLBUG_NECTAR][bug_at],
    ]
    .into_iter()
    .map(Spawned::new)
    .collect()
}

/// `BowlbugsNormal`. The Rock, then two workers by `NextItem`, each drawn
/// from those `_workerValidCounts` still allows. Each worker is allowed once,
/// so the second draw is over the two the first left, in the table's order.
fn bowlbugs_normal<V: crate::Build, R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let mut workers = vec![
        V::Monster::BOWLBUG_EGG,
        V::Monster::BOWLBUG_SILK,
        V::Monster::BOWLBUG_NECTAR,
    ];
    let mut roster = vec![V::Monster::BOWLBUG_ROCK];
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
fn flyconid_normal<V: crate::Build, R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let medium_at: usize = num_traits::cast(
        rng.next_int(Bound::try_new(R::Int::ONE + R::Int::ONE).expect("two medium slimes")),
    )
    .expect("a two-way draw is zero or one");
    [
        [V::Monster::LEAF_SLIME_M, V::Monster::TWIG_SLIME_M][medium_at],
        V::Monster::FLYCONID,
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
fn slimes_normal<V: crate::Build, R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let (third, fourth) = match rng.next_bool() {
        true => (V::Monster::LEAF_SLIME_S, V::Monster::TWIG_SLIME_S),
        false => (V::Monster::TWIG_SLIME_S, V::Monster::LEAF_SLIME_S),
    };
    [
        V::Monster::TWIG_SLIME_M,
        V::Monster::LEAF_SLIME_M,
        third,
        fourth,
    ]
    .into_iter()
    .map(Spawned::new)
    .collect()
}

/// `RubyRaidersNormal`. Three raiders, no two the same.
///
/// Each is one `NextItem` over the five, less those already standing, in the
/// order the game's own table lists them.
fn ruby_raiders_normal<V: crate::Build, R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let raiders: [V::Monster; 5] = [
        V::Monster::AXE_RUBY_RAIDER,
        V::Monster::ASSASSIN_RUBY_RAIDER,
        V::Monster::BRUTE_RUBY_RAIDER,
        V::Monster::CROSSBOW_RUBY_RAIDER,
        V::Monster::TRACKER_RUBY_RAIDER,
    ];

    let mut standing: Vec<V::Monster> = Vec::with_capacity(3);
    for _ in 0..3 {
        let left: Vec<V::Monster> = raiders
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
fn slithering_strangler_normal<V: crate::Build, R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let small: [V::Monster; 2] = [V::Monster::LEAF_SLIME_S, V::Monster::TWIG_SLIME_S];
    let medium: [V::Monster; 2] = [V::Monster::LEAF_SLIME_M, V::Monster::TWIG_SLIME_M];

    let kind: usize = num_traits::cast(
        rng.next_int(Bound::try_new(three::<R>()).expect("three kinds of company")),
    )
    .expect("a three-way draw is a valid kind");
    let mut roster = match kind {
        0 => vec![V::Monster::SNAPPING_JAXFRUIT],
        1 => vec![*rng.choose(&medium).expect("two medium slimes")],
        _ => vec![
            *rng.choose(&small).expect("two small slimes"),
            *rng.choose(&small).expect("two small slimes"),
        ],
    };
    roster.push(V::Monster::SLITHERING_STRANGLER);
    roster.into_iter().map(Spawned::new).collect()
}

/// `PunchOffEventEncounter`: two Punch Constructs, the first opening on its
/// fast punch. Each draws `NextInt(2, 10)` for the hit points it starts
/// without.
fn punch_off<V: crate::Build, R: GameRng>(rng: &mut R) -> Vec<Spawned<V>> {
    let bound = |n: i32| num_traits::cast(n).expect("a small number fits the RNG integer");
    let span = Span::new(bound(2), bound(10)).expect("the span is ordered");
    let mut reduction = || {
        let drawn: i32 =
            num_traits::cast(rng.next_int_range(span)).expect("the draw is inside the span");
        Some(drawn)
    };
    let first = Spawned {
        written: reduction(),
        ..Spawned::opening_on(V::Monster::PUNCH_CONSTRUCT, V::MoveId::FAST_PUNCH_MOVE)
    };
    let second = Spawned {
        written: reduction(),
        ..Spawned::new(V::Monster::PUNCH_CONSTRUCT)
    };
    vec![first, second]
}

/// `EncounterModel`'s own fields, for the encounters that keep any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// An encounter that saves nothing.
    Stateless,
    /// `GremlinMercNormal.GoldWasStolen`.
    GremlinMerc { gold_stolen: bool },
}

impl State {
    /// `GremlinMercNormal.MarkGoldStolen`, which another encounter does not
    /// have.
    pub(crate) fn mark_gold_stolen(&mut self) {
        if let Self::GremlinMerc { gold_stolen } = self {
            *gold_stolen = true;
        }
    }
}

impl<V: crate::Build> sts2_core::EncounterModel<V> for State {
    fn new(encounter: V::Encounter) -> Self {
        match encounter {
            __c0 if __c0 == V::Encounter::GREMLIN_MERC_NORMAL => {
                Self::GremlinMerc { gold_stolen: false }
            }
            _ => Self::Stateless,
        }
    }
}

/// `CalculateGoldProportion`, which `GremlinMercNormal` alone overrides. The
/// whole of it stands unless the Fat Gremlin escaped. Then it is half when
/// nothing was stolen, and none when gold was.
pub(crate) fn gold_proportion<V: crate::Build>(
    encounter: V::Encounter,
    state: &State,
    escaped: &[V::Monster],
) -> Option<f32> {
    match (encounter, *state) {
        (__c0, State::GremlinMerc { gold_stolen }) if __c0 == V::Encounter::GREMLIN_MERC_NORMAL => {
            Some(
                match (escaped.contains(&V::Monster::FAT_GREMLIN), gold_stolen) {
                    (false, _) => 1.0,
                    (true, false) => 0.5,
                    (true, true) => 0.0,
                },
            )
        }
        _ => None,
    }
}
