//! Creating the enemy creatures that begin a combat.

use crate::rng::MegaRng;
use smallvec::SmallVec;
use sts2_core::rng::{Choices, GameRng};
use sts2_core::{CombatId, ModelId, MoveMachine, Rules, SideKinds};

use super::Error;
use super::live::{CREATURES, Creatures, Fighter, HeldPower, Live, Stacked};
use super::powers::{PowerStore, Powers as _};
use crate::bounded::Bounded;
use crate::stream::Streams;

/// Roll a monster's health, avoiding healths already rolled in this fight
/// whenever its range leaves another choice.
fn health<V: Rules>(
    monster: V::Monster,
    ascension: V::AscensionLevel,
    creatures: &[Fighter<V>],
    niche: &mut MegaRng,
) -> V::Health {
    let range = V::monster_hp_bounds(monster, ascension);
    let used = creatures
        .iter()
        .filter_map(|creature| match creature.model {
            Some(ModelId::Monster(_)) => Some(creature.max_hp),
            _ => None,
        });
    let used: SmallVec<[V::Health; CREATURES]> = used.collect();
    // No monster in either build rolls in a range wider than eleven, so this
    // stays inline.
    let available: SmallVec<[V::Health; 16]> =
        range.values().filter(|hp| !used.contains(hp)).collect();
    if let Ok(choices) = Choices::try_new(&available) {
        *niche.choose_from(choices)
    } else {
        niche.next_interval(range)
    }
}

/// One monster's move graph, and where it stands.
///
/// Beside the fight rather than in it. An observation reports what a monster
/// does next, not how its graph reached that.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Machine<V: Rules> {
    /// The creature in the fight this graph belongs to.
    pub(crate) creature: CombatId,
    pub(crate) monster: V::Monster,
    /// What the encounter fixed when it built this one, which a conditional
    /// state in the graph asks about.
    pub(crate) spawn: sts2_core::MonsterSpawn<V::Encounter>,
    /// `SlotName`, as its place in `Encounter.Slots`, for a monster that
    /// stands in one.
    pub(crate) slot: Option<u8>,
    pub(crate) machine: MoveMachine,
}

impl<V: Rules> Clone for Machine<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for Machine<V> {}

/// Roll health and the first move in encounter order, advancing the two run
/// streams that own those draws.
pub(crate) fn initialize<V: Rules>(
    ascension: V::AscensionLevel,
    encounter: V::Encounter,
    streams: &mut Streams<V>,
    monsters: &[sts2_core::Spawned<V>],
    creatures: &mut Creatures<V>,
    powers: &mut PowerStore<V>,
    machines: &mut Bounded<Machine<V>, CREATURES>,
) -> Result<(), Error> {
    let rngs = streams.combat();

    // A fight the encounter builds nobody for is not a fight.
    let roster_size = u8::try_from(monsters.len())
        .ok()
        .and_then(nonmax::NonMaxU8::new)
        .ok_or(Error::CreatureIdOverflow)?;

    // The monsters follow whoever the fight already holds, and a health is
    // rolled against theirs alone.
    let first = creatures.len();
    for (
        index,
        &sts2_core::Spawned {
            monster,
            opens_on,
            slot,
        },
    ) in monsters.iter().enumerate()
    {
        let hp = health::<V>(monster, ascension, &creatures[first..], rngs.niche);

        let position = u8::try_from(index)
            .ok()
            .and_then(nonmax::NonMaxU8::new)
            .ok_or(Error::CreatureIdOverflow)?;
        // `CombatId` counts from the player, so the monsters start at one.
        let id = position
            .get()
            .checked_add(1)
            .and_then(nonmax::NonMaxU8::new)
            .map(CombatId::new)
            .ok_or(Error::CreatureIdOverflow)?;
        // The whole roster stands as each opening rolls.
        let spawn = sts2_core::MonsterSpawn {
            encounter,
            position,
            roster_size,
            standing: roster_size.get(),
        };
        let mut machine = V::opening_move_machine(monster, opens_on);
        let opening = V::roll_monster_move(
            monster,
            ascension,
            spawn,
            std::iter::empty::<super::live::Heard<'_, V>>(),
            &mut machine,
            rngs.ai,
        )?;
        machines.push(Machine {
            creature: id,
            monster,
            spawn,
            slot,
            machine,
        });
        creatures.push(Fighter {
            id,
            model: Some(ModelId::Monster(monster)),
            side: <V::CombatSide as SideKinds>::ENEMY,
            hp,
            max_hp: hp,
            block: num_traits::zero(),
            alive: true,
            // A fresh machine stands on no built state, so the walk always
            // names the move a monster opens on.
            next_move: opening.as_ref().map(Into::into),
            owner: None,
        });
        // A fight opens on the player's side.
        let side = <V::CombatSide as SideKinds>::PLAYER;
        for (power, amount) in V::monster_starting_powers(monster, ascension, side) {
            powers.push(id, HeldPower::new(power, Stacked::from(amount), Some(id)));
        }
    }

    // `AfterCreatureAdded`, which runs once every creature the encounter built
    // exists: each reads what the rest of its row stands at, and each writes
    // before the next one reads. The powers above are applied in the same
    // hook, and none of them reads another creature, so where they sit among
    // these two passes cannot be seen.
    for at in first..creatures.len() {
        let Some(ModelId::Monster(monster)) = creatures[at].model else {
            continue;
        };
        let side = creatures[at].side;
        let teammates: Vec<V::Health> = creatures
            .iter()
            .enumerate()
            .filter(|(other, creature)| *other != at && creature.side == side)
            .map(|(_, creature)| creature.max_hp)
            .collect();
        let Some(health) =
            V::monster_health_after_added(monster, ascension, creatures[at].max_hp, &teammates)
        else {
            continue;
        };
        // `SetMaxAndCurrentHp`, which is both numbers at once.
        creatures[at].max_hp = health;
        creatures[at].hp = health;
    }

    Ok(())
}

/// Add a monster during a fight, before the enemy side prepares its next turn.
///
/// # Panics
///
/// A listener to `AfterCreatureAddedToCombat`, which `CreatureCmd.Add` ends on.
pub(super) fn summon<V: Rules>(
    live: &mut Live<V>,
    relics: &crate::run::Belt<V>,
    monster: V::Monster,
    position: usize,
    slot: Option<u8>,
    niche: &mut MegaRng,
) -> CombatId {
    let encounter = live.encounter;
    let Some(roster_size) = live
        .machines
        .len()
        .checked_add(1)
        .and_then(|size| u8::try_from(size).ok())
        .and_then(nonmax::NonMaxU8::new)
    else {
        panic!("combat has too many creatures to assign an id");
    };
    let position = position.min(live.machines.len());
    let Some(spawn_position) = u8::try_from(position).ok().and_then(nonmax::NonMaxU8::new) else {
        unreachable!("a position in the bounded roster fits its spawn field");
    };
    let Some(id) = live
        .creatures
        .iter()
        .map(|creature| creature.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .and_then(nonmax::NonMaxU8::new)
        .map(CombatId::new)
    else {
        panic!("combat has too many creatures to assign an id");
    };
    let hp = health::<V>(monster, live.ascension, &live.creatures, niche);
    let creature_at = live
        .creatures
        .iter()
        .enumerate()
        .filter(|(_, creature)| creature.side == <V::CombatSide as SideKinds>::ENEMY)
        .nth(position)
        .map(|(at, _)| at)
        .unwrap_or(live.creatures.len());

    live.creatures.insert(
        creature_at,
        Fighter {
            id,
            model: Some(ModelId::Monster(monster)),
            side: <V::CombatSide as SideKinds>::ENEMY,
            hp,
            max_hp: hp,
            block: num_traits::zero(),
            alive: true,
            next_move: None,
            owner: None,
        },
    );
    live.machines.insert(
        position,
        Machine {
            creature: id,
            monster,
            spawn: sts2_core::MonsterSpawn {
                encounter,
                position: spawn_position,
                roster_size,
                standing: roster_size.get(),
            },
            slot,
            // Nothing writes onto a monster a fight summons: the
            // roster that could have is long built.
            machine: V::opening_move_machine(monster, None),
        },
    );
    for (position, held) in live.machines.iter_mut().enumerate() {
        let Some(position) = u8::try_from(position).ok().and_then(nonmax::NonMaxU8::new) else {
            unreachable!("a position in the bounded roster fits its spawn field");
        };
        held.spawn.position = position;
        held.spawn.roster_size = roster_size;
    }
    if super::listen::heard::<V>(
        live,
        relics,
        sts2_core::ModelHook::AfterCreatureAddedToCombat,
    ) {
        unimplemented!("a listener to a creature added to the fight");
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::live::Telegraphed;
    use sts2_content::v0_107_1::{
        V0_107_1,
        ascension::AscensionLevel,
        encounters::Encounter,
        monsters::{IntentKind, Monster, MoveId},
        runstate::RunRngType,
    };
    use sts2_core::rng::stream_seed;
    use sts2_core::{Attack, Intent, NextMove, Seed};

    /// `GenerateMonsters` refuses a number it already gave out. Three Inklets
    /// share one range and so get three healths.
    ///
    /// The slimes above cannot show it. Their ranges barely overlap, so the
    /// rule never sees a repeat.
    ///
    /// The numbers are the game's, off a `fight INKLETS_NORMAL` transcript.
    /// Not a scenario: both act-one encounters with monsters of a kind flag
    /// one of them, and branch its graph on the flag.
    #[test]
    fn monsters_of_a_kind_get_different_health() {
        let seed = Seed::new("0RACLE1").unwrap();
        let mut streams = Streams::<V0_107_1>::opening(stream_seed::<V0_107_1>(&seed));
        let inklets =
            [Monster::Inklet, Monster::Inklet, Monster::Inklet].map(sts2_core::Spawned::new);

        let (mut creatures, mut powers, mut machines) =
            (Creatures::new(), PowerStore::new(), Bounded::new());
        initialize::<V0_107_1>(
            AscensionLevel::None,
            Encounter::SlimesWeak,
            &mut streams,
            &inklets,
            &mut creatures,
            &mut powers,
            &mut machines,
        )
        .unwrap();

        assert_eq!(
            creatures.iter().map(|c| c.max_hp).collect::<Vec<_>>(),
            [13, 17, 12]
        );
    }

    /// `SetMoveImmediate` is a no-op on a move the graph cannot leave yet.
    ///
    /// `REATTACH_MOVE` is the segment's one `MustPerformOnceBeforeTransitioning`
    /// state. A segment killed while it stands there goes on reattaching, and
    /// reaches its dead move no earlier than the turn after.
    ///
    /// The replay does not reach this, because nothing killed a segment on the
    /// turn it was reattaching.
    #[test]
    fn a_move_that_has_to_be_performed_is_not_replaced() {
        use sts2_core::MovePinned;

        let segment = Monster::DecimillipedeSegmentFront;
        let mut machine = V0_107_1::opening_move_machine(segment, Some(MoveId::ReattachMove));
        let standing = machine.at;

        assert_eq!(
            V0_107_1::set_move_immediate(
                segment,
                AscensionLevel::None,
                MoveId::DeadMove,
                &mut machine
            ),
            Err(MovePinned),
            "the reattach has not been performed"
        );
        assert_eq!(machine.at, standing, "and the graph did not move");

        machine.performed();
        let next = V0_107_1::set_move_immediate(
            segment,
            AscensionLevel::None,
            MoveId::DeadMove,
            &mut machine,
        )
        .expect("a performed move can be left");
        assert_eq!(next.id, MoveId::DeadMove);
        assert!(
            !machine.log.contains(machine.at),
            "`ForceCurrentState` does not write the state into the log"
        );
    }

    /// `DecimillipedeSegment.AfterAddedToRoom` rewrites the rolled health.
    ///
    /// Each segment is even, and none carries what another already does. The
    /// third here rounds up onto the first's number, walks past the top of its
    /// own range, and wraps to the bottom of it.
    ///
    /// The numbers are the game's, off the `0RACLE1-REGENT-2` replay at the
    /// floor that elite stands on.
    #[test]
    fn the_decimillipede_segments_carry_different_even_healths() {
        let seed = Seed::new("0RACLE1").unwrap();
        let mut streams = Streams::<V0_107_1>::opening(stream_seed::<V0_107_1>(&seed));
        // The rolled numbers this reaches are 46, 43 and 45.
        for _ in 0..46 {
            streams.combat().niche.next_u64();
        }
        let segments = [
            Monster::DecimillipedeSegmentFront,
            Monster::DecimillipedeSegmentMiddle,
            Monster::DecimillipedeSegmentBack,
        ]
        .map(sts2_core::Spawned::new);

        let (mut creatures, mut powers, mut machines) =
            (Creatures::new(), PowerStore::new(), Bounded::new());
        initialize::<V0_107_1>(
            AscensionLevel::None,
            Encounter::DecimillipedeElite,
            &mut streams,
            &segments,
            &mut creatures,
            &mut powers,
            &mut machines,
        )
        .unwrap();

        assert_eq!(
            creatures
                .iter()
                .map(|c| (c.hp, c.max_hp))
                .collect::<Vec<_>>(),
            [(46, 46), (44, 44), (40, 40)]
        );
    }

    #[test]
    fn the_oracle_weak_slimes_get_the_same_health_and_moves() {
        let seed = Seed::new("0RACLE1").unwrap();
        let mut streams = Streams::<V0_107_1>::opening(stream_seed::<V0_107_1>(&seed));
        let monsters = [
            Monster::TwigSlimeS,
            Monster::LeafSlimeM,
            Monster::LeafSlimeS,
        ]
        .map(sts2_core::Spawned::new);

        let (mut creatures, mut powers, mut machines) =
            (Creatures::new(), PowerStore::new(), Bounded::new());
        initialize::<V0_107_1>(
            AscensionLevel::None,
            Encounter::SlimesWeak,
            &mut streams,
            &monsters,
            &mut creatures,
            &mut powers,
            &mut machines,
        )
        .unwrap();

        assert_eq!(
            creatures.iter().map(|c| c.max_hp).collect::<Vec<_>>(),
            [8, 35, 12]
        );
        let at = streams.positions();
        assert_eq!(at.run[&RunRngType::Niche].counter, 3);
        assert_eq!(at.run[&RunRngType::MonsterAi].counter, 1);
        assert_eq!(
            creatures
                .iter()
                .map(|c| c.next_move.as_ref().map(Telegraphed::telegraph))
                .collect::<Vec<_>>(),
            vec![
                Some(NextMove {
                    id: MoveId::TackleMove,
                    intents: smallvec::smallvec![Intent {
                        kind: IntentKind::Attack,
                        attack: Some(Attack {
                            damage: 4,
                            repeats: 1
                        }),
                    }],
                }),
                Some(NextMove {
                    id: MoveId::StickyShot,
                    intents: smallvec::smallvec![Intent {
                        kind: IntentKind::StatusCard,
                        attack: None
                    }]
                }),
                Some(NextMove {
                    id: MoveId::GoopMove,
                    intents: smallvec::smallvec![Intent {
                        kind: IntentKind::StatusCard,
                        attack: None
                    }]
                }),
            ]
        );
    }
}
