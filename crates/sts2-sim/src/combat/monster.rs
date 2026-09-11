//! Creating the enemy creatures that begin a combat.

use crate::rng::MegaRng;
use sts2_core::rng::GameRng;
use sts2_core::{CombatId, Counters, Creature, ModelId, MoveMachine, Rules, SideKinds};

use super::Error;
use super::live::Live;

/// Roll a monster's health, avoiding healths already rolled in this fight
/// whenever its range leaves another choice.
fn health<V: Rules>(
    monster: V::Monster,
    ascension: V::AscensionLevel,
    creatures: &[Creature<V>],
    niche: &mut MegaRng,
) -> u16 {
    let range = V::monster_hp_bounds(monster, ascension);
    let (min, max) = (range.min(), range.max());
    let used = creatures
        .iter()
        .filter_map(|creature| match creature.model {
            Some(ModelId::Monster(_)) => num_traits::cast(creature.max_hp),
            _ => None,
        });
    let used: Vec<u16> = used.collect();
    let available: Vec<u16> = (min..=max).filter(|hp| !used.contains(hp)).collect();
    let choices: Vec<u16> = if available.is_empty() {
        (min..=max).collect()
    } else {
        available
    };
    let Ok(&hp) = niche.choose(&choices) else {
        unreachable!("a generated monster HP interval has a choice");
    };
    hp
}

/// A table's monster health in the number type that table chose.
fn number<V: Rules>(hp: u16) -> V::Number {
    let Some(hp) = num_traits::cast(hp) else {
        unreachable!("a build's number type holds its generated monster HP");
    };
    hp
}

/// The enemy side of a fight, as it begins.
#[derive(Debug)]
pub(crate) struct Roster<V: Rules> {
    pub(crate) creatures: Vec<Creature<V>>,
    pub(crate) machines: Vec<Machine<V>>,
}

/// One monster's move graph, and where it stands.
///
/// Beside the fight rather than in it. An observation reports what a monster
/// does next, not how its graph reached that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Machine<V: Rules> {
    /// The creature in the fight this graph belongs to.
    pub(crate) creature: CombatId,
    pub(crate) monster: V::Monster,
    /// What the encounter fixed when it built this one, which a conditional
    /// state in the graph asks about.
    pub(crate) spawn: sts2_core::MonsterSpawn<V::Encounter>,
    pub(crate) machine: MoveMachine,
}

/// Roll health and the first move in encounter order, advancing the two run
/// streams that own those draws.
pub(crate) fn initialize<V: Rules>(
    raw_seed: u32,
    ascension: V::AscensionLevel,
    encounter: V::Encounter,
    counters: &mut Counters<V>,
    monsters: &[V::Monster],
) -> Result<Roster<V>, Error> {
    let mut niche = super::stream::seek(raw_seed, counters, V::niche_rng(), "niche")?;
    let mut ai = super::stream::seek(raw_seed, counters, V::monster_ai_rng(), "monster_ai")?;

    // A fight the encounter builds nobody for is not a fight.
    let roster_size = u8::try_from(monsters.len())
        .ok()
        .and_then(nonmax::NonMaxU8::new)
        .ok_or(Error::CreatureIdOverflow)?;

    let mut creatures = Vec::with_capacity(monsters.len());
    let mut machines = Vec::with_capacity(monsters.len());
    for (index, &monster) in monsters.iter().enumerate() {
        let hp = number::<V>(health::<V>(monster, ascension, &creatures, &mut niche));

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
        let spawn = sts2_core::MonsterSpawn {
            encounter,
            position,
            roster_size,
        };
        let mut machine = V::opening_move_machine(monster);
        let opening = V::roll_monster_move(monster, ascension, spawn, &mut machine, &mut ai)?;
        machines.push(Machine {
            creature: id,
            monster,
            spawn,
            machine,
        });
        creatures.push(Creature {
            id,
            model: Some(ModelId::Monster(monster)),
            side: <V::CombatSide as SideKinds>::ENEMY,
            hp,
            max_hp: hp,
            block: num_traits::zero(),
            alive: true,
            powers: Vec::new(),
            next_move: Some(opening),
        });
    }

    super::stream::record(counters, V::niche_rng(), "niche", &niche)?;
    super::stream::record(counters, V::monster_ai_rng(), "monster_ai", &ai)?;
    Ok(Roster {
        creatures,
        machines,
    })
}

/// Add a monster during a fight, before the enemy side prepares its next turn.
pub(super) fn summon<V: Rules>(
    live: &mut Live<V>,
    monster: V::Monster,
    position: usize,
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
    let hp = number::<V>(health::<V>(monster, live.ascension, &live.creatures, niche));
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
        Creature {
            id,
            model: Some(ModelId::Monster(monster)),
            side: <V::CombatSide as SideKinds>::ENEMY,
            hp,
            max_hp: hp,
            block: num_traits::zero(),
            alive: true,
            powers: Vec::new(),
            next_move: None,
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
            },
            machine: V::opening_move_machine(monster),
        },
    );
    for (position, held) in live.machines.iter_mut().enumerate() {
        let Some(position) = u8::try_from(position).ok().and_then(nonmax::NonMaxU8::new) else {
            unreachable!("a position in the bounded roster fits its spawn field");
        };
        held.spawn.position = position;
        held.spawn.roster_size = roster_size;
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use sts2_content::v0_107_1::{
        V0_107_1,
        ascension::AscensionLevel,
        encounters::Encounter,
        monsters::{IntentKind, Monster, MoveId},
        runstate::{PlayerRngType, RunRngType},
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
        let mut counters = Counters {
            run: RunRngType::ALL.into_iter().map(|kind| (kind, 0)).collect(),
            player: HashMap::<PlayerRngType, u32>::new(),
        };
        let inklets = [Monster::Inklet, Monster::Inklet, Monster::Inklet];

        let roster = initialize::<V0_107_1>(
            stream_seed(&seed),
            AscensionLevel::None,
            Encounter::SlimesWeak,
            &mut counters,
            &inklets,
        )
        .unwrap();

        assert_eq!(
            roster
                .creatures
                .iter()
                .map(|c| c.max_hp)
                .collect::<Vec<_>>(),
            [13, 17, 12]
        );
    }

    #[test]
    fn the_oracle_weak_slimes_get_the_same_health_and_moves() {
        let seed = Seed::new("0RACLE1").unwrap();
        let mut counters = Counters {
            run: RunRngType::ALL.into_iter().map(|kind| (kind, 0)).collect(),
            player: HashMap::<PlayerRngType, u32>::new(),
        };
        let monsters = [
            Monster::TwigSlimeS,
            Monster::LeafSlimeM,
            Monster::LeafSlimeS,
        ];

        let roster = initialize::<V0_107_1>(
            stream_seed(&seed),
            AscensionLevel::None,
            Encounter::SlimesWeak,
            &mut counters,
            &monsters,
        )
        .unwrap();
        let creatures = roster.creatures;

        assert_eq!(
            creatures.iter().map(|c| c.max_hp).collect::<Vec<_>>(),
            [8, 35, 12]
        );
        assert_eq!(counters.run[&RunRngType::Niche], 3);
        assert_eq!(counters.run[&RunRngType::MonsterAi], 1);
        assert_eq!(
            creatures
                .iter()
                .map(|c| c.next_move.clone())
                .collect::<Vec<_>>(),
            vec![
                Some(NextMove {
                    id: MoveId::TackleMove,
                    intents: vec![Intent {
                        kind: IntentKind::Attack,
                        attack: Some(Attack {
                            damage: 4,
                            repeats: 1
                        }),
                    }],
                }),
                Some(NextMove {
                    id: MoveId::StickyShot,
                    intents: vec![Intent {
                        kind: IntentKind::StatusCard,
                        attack: None
                    }]
                }),
                Some(NextMove {
                    id: MoveId::GoopMove,
                    intents: vec![Intent {
                        kind: IntentKind::StatusCard,
                        attack: None
                    }]
                }),
            ]
        );
    }
}

#[cfg(test)]
mod counter_tests {
    use super::*;
    use std::collections::HashMap;
    use sts2_content::v0_107_1::{
        V0_107_1,
        ascension::AscensionLevel,
        encounters::Encounter,
        monsters::Monster,
        runstate::{PlayerRngType, RunRngType},
    };

    #[test]
    fn a_missing_required_stream_is_not_counter_zero() {
        for (missing, stream) in [
            (RunRngType::Niche, "niche"),
            (RunRngType::MonsterAi, "monster_ai"),
        ] {
            let mut counters = Counters {
                run: RunRngType::ALL
                    .into_iter()
                    .filter(|kind| *kind != missing)
                    .map(|kind| (kind, 0))
                    .collect(),
                player: HashMap::<PlayerRngType, u32>::new(),
            };
            let before = counters.clone();

            let error = initialize::<V0_107_1>(
                123,
                AscensionLevel::None,
                Encounter::SlimesWeak,
                &mut counters,
                &[Monster::TwigSlimeS],
            )
            .expect_err("a required counter is absent");
            assert_eq!(error, Error::MissingCounter { stream });
            assert_eq!(counters, before);
        }
    }
}
