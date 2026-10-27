//! Creating the enemy creatures that begin a combat.

use crate::rng::MegaRng;
use smallvec::SmallVec;
use sts2_core::rng::{Choices, GameRng};
use sts2_core::{CombatId, EncounterTable as _, ModelId, MoveMachine, Rules, SideKinds};

use super::Error;
use super::live::{CREATURES, Creatures, Fighter, HeldPower, Live, Stacked};
use super::powers::{PowerStore, Powers as _};
use crate::bounded::Bounded;
use crate::stream::Streams;

/// The model a spawned monster joins with: what the roster wrote on it, or
/// a new one.
fn spawned_model<V: Rules>(spawned: &sts2_core::Spawned<V>) -> V::MonsterState {
    match spawned.written {
        Some(written) => sts2_core::MonsterModel::written(spawned.monster, written),
        None => sts2_core::MonsterModel::new(spawned.monster),
    }
}

/// Roll a monster's health, avoiding healths already rolled in this fight
/// whenever its range leaves another choice.
fn health<V: Rules>(
    monster: V::Monster,
    model: &V::MonsterState,
    ascension: V::AscensionLevel,
    used: &[V::Health],
    niche: &mut MegaRng,
) -> V::Health {
    let range = V::monster_hp_bounds(monster, model, ascension);
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

/// The monsters a fight opens with, and the health each already rolled,
/// where an event rolled it as it opened. Empty for a fight that rolls its
/// own.
#[derive(Clone, Copy)]
pub(crate) struct Roster<'a, V: Rules> {
    pub(crate) monsters: &'a [sts2_core::Spawned<V>],
    pub(crate) rolled: &'a [V::Health],
}

/// The maximum health of each enemy among `creatures`, which a roll keeps
/// clear of. Only the enemy side's list counts, so Osty's is no bar.
fn monster_hps<V: Rules>(creatures: &[Fighter<V>]) -> SmallVec<[V::Health; CREATURES]> {
    creatures
        .iter()
        .filter(|creature| creature.side == <V::CombatSide as SideKinds>::ENEMY)
        .map(|creature| creature.max_hp)
        .collect()
}

/// `GenerateInternalCombatState`: the health each of an encounter's
/// monsters rolls as a `Combat`-layout event builds its fight, in the order
/// they stand. `CombatState.CreateCreature` rolls it off `Niche` against the
/// ones before.
pub(crate) fn roll_layout<V: Rules>(
    monsters: &[sts2_core::Spawned<V>],
    ascension: V::AscensionLevel,
    niche: &mut MegaRng,
) -> SmallVec<[V::Health; 2]> {
    let mut rolled: SmallVec<[V::Health; 2]> = SmallVec::new();
    for spawned in monsters {
        let hp = health::<V>(
            spawned.monster,
            &spawned_model(spawned),
            ascension,
            &rolled,
            niche,
        );
        rolled.push(hp);
    }
    rolled
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
    /// What the monster's own model keeps between its moves.
    pub(crate) state: V::MonsterState,
    /// `MonsterModel.SpawnedThisTurn`: set as it is set up, and cleared as
    /// the sides switch. A monster that comes in during its side's turn
    /// takes no turn then.
    pub(crate) spawned_this_turn: bool,
}

impl<V: Rules> Clone for Machine<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for Machine<V> {}

/// `MultiplayerScalingModel` for a fight of `players`: how much more health a
/// monster has with more than one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Scaling {
    pub(crate) players: usize,
    /// `GetMultiplayerScaling`: the act's factor.
    pub(crate) factor: sts2_core::Decimal,
}

impl Scaling {
    /// The scaling of a fight of `players` in the act `act`, counted from
    /// zero. The factor is 1.1 in the first act and 1.2 in the second. In
    /// the third it is 1.3 for a boss and 1.2 otherwise.
    pub(crate) fn of<V: Rules>(players: usize, encounter: V::Encounter, act: usize) -> Self {
        use sts2_core::{EncounterTable as _, RoomKinds as _};

        let tenths = match act {
            0 => 11,
            1 => 12,
            2 if encounter.room() == <V::Encounter as sts2_core::EncounterTable>::Room::BOSS => 13,
            2 => 12,
            act => unimplemented!("a fight of several players in act {act}"),
        };
        Scaling {
            players,
            factor: sts2_core::Decimal::new(tenths, 1),
        }
    }

    /// `ScaleMonsterHpForMultiplayer`: `hp * players * factor`, truncated
    /// as `SetMaxHpInternal` stores it.
    pub(crate) fn health<V: Rules>(self, hp: V::Health) -> V::Health {
        if self.players <= 1 {
            return hp;
        }
        let whole: sts2_core::Decimal = num_traits::cast::<_, i64>(hp)
            .expect("a monster's health fits an i64")
            .into();
        let scaled = whole * sts2_core::Decimal::from(self.players) * self.factor;
        num_traits::cast(
            num_traits::ToPrimitive::to_i64(&scaled.trunc()).expect("a scaled health fits an i64"),
        )
        .expect("a scaled health fits the build's health type")
    }
}

/// Roll health and the first move in encounter order, advancing the two run
/// streams that own those draws.
pub(crate) fn initialize<V: Rules>(
    ascension: V::AscensionLevel,
    scaling: Scaling,
    encounter: V::Encounter,
    streams: &mut Streams<V>,
    roster: Roster<'_, V>,
    creatures: &mut Creatures<V>,
    machines: &mut Bounded<Machine<V>, CREATURES>,
) -> Result<(), Error> {
    let Roster { monsters, rolled } = roster;
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
            written: _,
        },
    ) in monsters.iter().enumerate()
    {
        let state = spawned_model(&monsters[index]);
        // A fight an event built as it opened rolled its health then.
        let hp = match rolled.get(index) {
            Some(_) if scaling.players > 1 => {
                unimplemented!("health an event rolled in a fight of several players")
            }
            Some(&hp) => hp,
            // `SetUniqueMonsterHpValue`, against the health of those standing
            // already, then `ScaleMonsterHpForMultiplayer`.
            None => {
                let used = monster_hps::<V>(&creatures[first..]);
                scaling.health::<V>(health::<V>(monster, &state, ascension, &used, rngs.niche))
            }
        };

        let position = u8::try_from(index)
            .ok()
            .and_then(nonmax::NonMaxU8::new)
            .ok_or(Error::CreatureIdOverflow)?;
        // `CombatId` counts from the players, so the monsters start after them.
        let id = position
            .get()
            .checked_add(u8::try_from(first).expect("a fight's players fit a u8"))
            .and_then(nonmax::NonMaxU8::new)
            .map(CombatId::new)
            .ok_or(Error::CreatureIdOverflow)?;
        // The whole roster stands as each opening rolls.
        let whole: u32 = num_traits::cast(hp).expect("a monster's health fits a u32");
        let spawn = sts2_core::MonsterSpawn {
            encounter,
            position,
            roster_size,
            standing: roster_size.get(),
            hp: whole,
            max_hp: whole,
            next_slot: (0..encounter.slots())
                .any(|free| monsters.iter().all(|spawned| spawned.slot != Some(free))),
        };
        let mut machine = V::opening_move_machine(monster, opens_on);
        let opening = V::roll_monster_move(
            monster,
            ascension,
            spawn,
            std::iter::empty::<super::live::Heard<'_, V>>(),
            // Those ahead of it in the roster rolled theirs already.
            |wanted| {
                creatures[first..].iter().any(|other| {
                    other
                        .next_move
                        .as_ref()
                        .is_some_and(|next| next.id == wanted)
                })
            },
            &state,
            &mut machine,
            rngs.ai,
        )?;
        machines.push(Machine {
            creature: id,
            monster,
            spawn,
            slot,
            machine,
            state,
            spawned_this_turn: true,
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
    }
    Ok(())
}

/// `CombatManager.StartCombatInternal`'s `AfterCreatureAdded` for the
/// monsters [`initialize`] built: `AfterAddedToRoom` gives each its starting
/// powers, and its opponents theirs. `SetUpCombat` only queues it, so it runs
/// after `AfterRoomEntered`.
///
/// Each monster then reads its row's health and writes its own, and then its
/// current hit points alone. No power reads another creature, so the passes'
/// order cannot be seen.
pub(crate) fn added_to_room<V: Rules>(
    ascension: V::AscensionLevel,
    creatures: &mut Creatures<V>,
    powers: &mut PowerStore<V>,
    machines: &Bounded<Machine<V>, CREATURES>,
) {
    for held in machines.iter() {
        let (id, monster) = (held.creature, held.monster);
        // A fight opens on the player's side.
        let side = <V::CombatSide as SideKinds>::PLAYER;
        for (power, amount) in V::monster_starting_powers(monster, ascension, side, &held.state) {
            starting::<V>(powers, id, power, Stacked::from(amount), Some(id));
        }
        let opponents: SmallVec<[CombatId; 2]> = creatures
            .iter()
            .filter(|creature| creature.side != <V::CombatSide as SideKinds>::ENEMY)
            .map(|creature| creature.id)
            .collect();
        for (power, amount) in V::monster_starting_opponent_powers(monster, ascension) {
            for &opponent in &opponents {
                starting::<V>(powers, opponent, power, Stacked::from(amount), Some(id));
            }
        }
    }
    for at in 0..creatures.len() {
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
    for held in machines.iter() {
        let Some(at) = creatures
            .iter()
            .position(|creature| creature.id == held.creature)
        else {
            continue;
        };
        if let Some(hp) = V::monster_hp_after_added(held.monster, &held.state, creatures[at].hp) {
            creatures[at].hp = hp;
        }
    }
}

/// `PowerCmd.Apply` of a starting power onto `creature`. Only what a room's
/// `AfterRoomEntered` landed stands there before it, and a counter stacks
/// into that instance through `ModifyAmount`.
fn starting<V: Rules>(
    powers: &mut PowerStore<V>,
    creature: CombatId,
    power: V::Power,
    amount: Stacked,
    applier: Option<CombatId>,
) {
    let per_applier = sts2_core::PowerTable::per_applier(power);
    let fresh = sts2_core::PowerTable::instanced(power) && !per_applier;
    let stacks =
        |held: &HeldPower<V>| held.power() == power && (!per_applier || held.applier == applier);
    if !fresh && let Some(held) = powers.of_mut(creature).find(|held| stacks(held)) {
        let (Some(had), Some(more)) = (held.amount.count(), amount.count()) else {
            unimplemented!("an endless {power:?} stacking as a monster arrives")
        };
        held.amount = Stacked::counted(had + more);
        return;
    }
    powers.push(creature, HeldPower::new(power, amount, applier));
}

/// Add a monster during a fight, before the enemy side prepares its next
/// turn. `CreatureCmd.Add` ends on `AfterCreatureAddedToCombat`, which the
/// board raises once it has rolled or set the newcomer's move.
pub(super) fn summon<V: Rules>(
    live: &mut Live<V>,
    monster: V::Monster,
    position: usize,
    slot: Option<u8>,
    niche: &mut MegaRng,
) -> CombatId {
    summon_built::<V>(
        live,
        sts2_core::Spawned {
            monster,
            opens_on: None,
            slot,
            written: None,
        },
        sts2_core::MonsterModel::new(monster),
        position,
        niche,
    )
}

/// [`summon`] of a monster whose model something wrote on before it joined,
/// opening where `spawned` says.
pub(super) fn summon_built<V: Rules>(
    live: &mut Live<V>,
    spawned: sts2_core::Spawned<V>,
    state: V::MonsterState,
    position: usize,
    niche: &mut MegaRng,
) -> CombatId {
    // What the roster wrote is already on `state`.
    let sts2_core::Spawned {
        monster,
        opens_on,
        slot,
        written: _,
    } = spawned;
    let (id, hp) = create::<V>(live, monster, &state, niche);
    place::<V>(live, monster, opens_on, slot, state, position, id, hp)
}

/// `CombatState.CreateCreature` for an enemy: the next id, and hit points
/// `SetUniqueMonsterHpValue` rolls against the enemies already in the fight.
pub(super) fn create<V: Rules>(
    live: &mut Live<V>,
    monster: V::Monster,
    model: &V::MonsterState,
    niche: &mut MegaRng,
) -> (CombatId, V::Health) {
    let id = live.next_creature();
    let used = monster_hps::<V>(&live.creatures);
    (
        id,
        health::<V>(monster, model, live.ascension, &used, niche),
    )
}

/// `CombatState.AddCreature`: a monster [`create`] made, put among the
/// enemies at `position`, opening where `opens_on` says.
#[allow(clippy::too_many_arguments)]
pub(super) fn place<V: Rules>(
    live: &mut Live<V>,
    monster: V::Monster,
    opens_on: Option<V::MoveId>,
    slot: Option<u8>,
    state: V::MonsterState,
    position: usize,
    id: CombatId,
    hp: V::Health,
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
                hp: num_traits::cast(hp).expect("a monster's health fits a u32"),
                max_hp: num_traits::cast(hp).expect("a monster's health fits a u32"),
                // Each roll reads it afresh.
                next_slot: false,
            },
            slot,
            machine: V::opening_move_machine(monster, opens_on),
            state,
            spawned_this_turn: true,
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
        let mut streams = Streams::<V0_107_1>::opening(stream_seed::<V0_107_1>(&seed), 1);
        let inklets =
            [Monster::Inklet, Monster::Inklet, Monster::Inklet].map(sts2_core::Spawned::new);

        let (mut creatures, mut powers, mut machines) =
            (Creatures::new(), PowerStore::new(), Bounded::new());
        initialize::<V0_107_1>(
            AscensionLevel::None,
            Scaling::of::<V0_107_1>(1, Encounter::SlimesWeak, 0),
            Encounter::SlimesWeak,
            &mut streams,
            Roster {
                monsters: &inklets,
                rolled: &[],
            },
            &mut creatures,
            &mut machines,
        )
        .unwrap();
        added_to_room::<V0_107_1>(AscensionLevel::None, &mut creatures, &mut powers, &machines);

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
                false,
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
            false,
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
        let mut streams = Streams::<V0_107_1>::opening(stream_seed::<V0_107_1>(&seed), 1);
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
            Scaling::of::<V0_107_1>(1, Encounter::DecimillipedeElite, 0),
            Encounter::DecimillipedeElite,
            &mut streams,
            Roster {
                monsters: &segments,
                rolled: &[],
            },
            &mut creatures,
            &mut machines,
        )
        .unwrap();
        added_to_room::<V0_107_1>(AscensionLevel::None, &mut creatures, &mut powers, &machines);

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
        let mut streams = Streams::<V0_107_1>::opening(stream_seed::<V0_107_1>(&seed), 1);
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
            Scaling::of::<V0_107_1>(1, Encounter::SlimesWeak, 0),
            Encounter::SlimesWeak,
            &mut streams,
            Roster {
                monsters: &monsters,
                rolled: &[],
            },
            &mut creatures,
            &mut machines,
        )
        .unwrap();
        added_to_room::<V0_107_1>(AscensionLevel::None, &mut creatures, &mut powers, &machines);

        assert_eq!(
            creatures.iter().map(|c| c.max_hp).collect::<Vec<_>>(),
            [8, 35, 12]
        );
        let at = streams.positions(sts2_core::SeatIndex(0));
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
