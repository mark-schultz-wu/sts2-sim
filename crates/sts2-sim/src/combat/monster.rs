//! Creating the enemy creatures that begin a combat.

use sts2_core::rng::{Bound, GameRng, MegaRng};
use sts2_core::{Counters, Creature, ModelId, Rules};

use super::Error;

/// Roll health and the first move in encounter order, advancing the two run
/// streams that own those draws.
pub(crate) fn initialize<V: Rules>(
    raw_seed: u32,
    ascension: V::AscensionLevel,
    counters: &mut Counters<V>,
    monsters: &[V::Monster],
) -> Result<Vec<Creature<V>>, Error> {
    let niche_at = counter(counters, V::niche_rng(), "niche")?;
    let ai_at = counter(counters, V::monster_ai_rng(), "monster_ai")?;
    let mut niche = MegaRng::named(raw_seed, "niche");
    let mut ai = MegaRng::named(raw_seed, "monster_ai");
    niche.fast_forward(niche_at).map_err(|source| Error::Seek {
        stream: "niche",
        source,
    })?;
    ai.fast_forward(ai_at).map_err(|source| Error::Seek {
        stream: "monster_ai",
        source,
    })?;

    let mut health = Vec::with_capacity(monsters.len());
    let mut creatures = Vec::with_capacity(monsters.len());
    for (index, &monster) in monsters.iter().enumerate() {
        let range = V::monster_hp_bounds(monster, ascension);
        let (min, max) = (range.min(), range.max());
        let available: Vec<u16> = (min..=max).filter(|hp| !health.contains(hp)).collect();
        let choices: Vec<u16> = if available.is_empty() {
            (min..=max).collect()
        } else {
            available
        };
        let count = i32::try_from(choices.len()).expect("a u16 interval fits in an i32");
        let bound = Bound::new(count).expect("an ordered inclusive interval is nonempty");
        let hp = choices[niche.next_int(bound) as usize];
        health.push(hp);

        let id = u32::try_from(index)
            .ok()
            .and_then(|id| id.checked_add(1))
            .ok_or(Error::CreatureIdOverflow)?;
        creatures.push(Creature {
            id,
            model: Some(ModelId::Monster(monster)),
            side: V::enemy_side(),
            hp: i32::from(hp),
            max_hp: i32::from(hp),
            block: 0,
            alive: true,
            powers: Vec::new(),
            next_move: Some(V::initial_monster_move(monster, ascension, &mut ai)?),
        });
    }

    let niche_counter =
        u32::try_from(niche.counter()).map_err(|_| Error::CounterOutOfRange { stream: "niche" })?;
    let ai_counter = u32::try_from(ai.counter()).map_err(|_| Error::CounterOutOfRange {
        stream: "monster_ai",
    })?;
    counters.run.insert(V::niche_rng(), niche_counter);
    counters.run.insert(V::monster_ai_rng(), ai_counter);
    Ok(creatures)
}

fn counter<V: Rules>(
    counters: &Counters<V>,
    kind: V::RunRngType,
    stream: &'static str,
) -> Result<i32, Error> {
    let counter = counters
        .run
        .get(&kind)
        .copied()
        .ok_or(Error::MissingCounter { stream })?;
    i32::try_from(counter).map_err(|_| Error::CounterOutOfRange { stream })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use sts2_content::v0_107_1::{
        V0_107_1,
        ascension::AscensionLevel,
        monsters::{IntentKind, Monster, MoveId},
        runstate::{PlayerRngType, RunRngType},
    };
    use sts2_core::rng::stream_seed;
    use sts2_core::{Attack, Intent, NextMove, Seed};

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

        let creatures = initialize::<V0_107_1>(
            stream_seed(&seed),
            AscensionLevel::None,
            &mut counters,
            &monsters,
        )
        .unwrap();

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
                &mut counters,
                &[Monster::TwigSlimeS],
            )
            .expect_err("a required counter is absent");
            assert_eq!(error, Error::MissingCounter { stream });
            assert_eq!(counters, before);
        }
    }
}
