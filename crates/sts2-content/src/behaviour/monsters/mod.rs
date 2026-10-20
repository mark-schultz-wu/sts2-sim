//! What a monster's move does, ported.
//!
//! `MoveState`'s `onPerform`, one function per move. One module per act, the
//! way `generated/monster_moves/` is filed, so a monster's moves sit under the
//! same name as its generated ones. A move routes by
//! [`monster_moves::filed`], and an act gets a module when its first monster
//! is ported.
//!
//! A monster nobody has ported panics naming itself, so this tree is also the
//! list of fights the simulator can take a turn in.

use sts2_core::{ContentError, EventHook, Fight, MonsterSpawn, Played};

use crate::generated::monster_moves::Filed;
use crate::ids::{EncounterIds as _, MonsterIds as _, MoveIds as _, PowerIds as _};

/// What a move's own numbers are held as.
pub(crate) type Number = i32;

pub(crate) mod glory;
pub(crate) mod hive;
mod overgrowth;
mod shared;
mod summoned;
mod underdocks;

/// Where a monster's move stopped on the player's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// The Knowledge Demon's `ChooseCurse`, for the set at this index.
    CurseOfKnowledge { set: u8 },
    /// A move whose attack stood still on a question the player had to
    /// answer, from this step once the answer is in.
    Step(u8),
}

/// Carry a monster's move on with the player's answer.
pub(crate) fn resume<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: Option<sts2_core::CardIdent<V>>,
) -> Played<V> {
    hive::resume::<V, _>(fight, at, chosen)
}

/// `IllusionPower.ReviveMove`: the illusion comes back whole.
///
/// `CreatureCmd.Heal(Owner, MaxHp - CurrentHp)`, which is what takes a dead
/// one back over zero.
fn revive<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    use sts2_core::CreatureView as _;

    let actor = fight.actor();
    let Some(missing) = fight
        .creatures()
        .find(|creature| creature.id() == actor)
        .map(|creature| creature.max_hp() - creature.hp())
    else {
        return;
    };
    fight.write_power(actor, V::Power::ILLUSION_POWER, |state| {
        state.set_reviving(false)
    });
    fight.heal(actor, missing);
}

/// Powers a monster applies to itself as it enters combat.
///
/// `AfterAddedToRoom`, where a monster that has one `PowerCmd.Apply`s on its
/// own creature. Most monsters do not override it, and enter with none. One
/// that does and has no port stops by name.
///
/// Filed by act beside the moves, so a monster's whole port is one file.
pub(crate) fn starting_powers<V: crate::Build>(
    monster: V::Monster,
    ascension: crate::generated::ascension::AscensionLevel,
    side: crate::generated::runstate::CombatSide,
    model: &Model<V>,
) -> Vec<(V::Power, sts2_core::PowerAmount<Number>)> {
    use sts2_core::MonsterTable as _;

    match overgrowth::starting_powers::<V>(monster, ascension)
        .or_else(|| hive::starting_powers::<V>(monster, ascension, side))
        .or_else(|| glory::starting_powers::<V>(monster, ascension, model))
        .or_else(|| shared::starting_powers::<V>(monster))
        .or_else(|| summoned::starting_powers::<V>(monster))
        .or_else(|| underdocks::starting_powers::<V>(monster, ascension))
    {
        Some(applied) => applied,
        None if !monster.overrides_virtual(sts2_core::MonsterVirtual::AfterAddedToRoom) => {
            Vec::new()
        }
        None => unimplemented!("what {monster:?} does as it enters the fight"),
    }
}

/// Powers a monster's `AfterAddedToRoom` applies to each of its opponents.
/// The Rocket surrounds the player.
pub(crate) fn starting_opponent_powers<V: crate::Build>(
    monster: V::Monster,
) -> Vec<(V::Power, sts2_core::PowerAmount<Number>)> {
    match monster {
        __c0 if __c0 == V::Monster::ROCKET => vec![(
            V::Power::SURROUNDED_POWER,
            sts2_core::PowerAmount::Counted(1),
        )],
        _ => Vec::new(),
    }
}

/// What `AfterAddedToRoom`'s `SetCurrentHpInternal` leaves a monster at, for
/// one that lowers its own.
pub(crate) fn hp_after_added<V: crate::Build>(
    monster: V::Monster,
    model: &Model<V>,
    hp: i32,
) -> Option<i32> {
    glory::hp_after_added::<V>(monster, model, hp)
}

/// What `AfterAddedToRoom` writes over the health a monster was built with.
///
/// Filed by act beside the moves, the way the starting powers are.
pub(crate) fn health_after_added<V: crate::Build>(
    monster: V::Monster,
    ascension: crate::generated::ascension::AscensionLevel,
    rolled: i32,
    teammates: &[i32],
) -> Option<i32> {
    hive::health_after_added::<V>(monster, ascension, rolled, teammates)
}

/// A move's attack damage that the monster works out from the fight, from
/// the powers on its own creature as the move rolls.
///
/// # Panics
///
/// A move nobody has ported.
pub(crate) fn computed_damage<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    monster: V::Monster,
    move_id: V::MoveId,
    ascension: u8,
    powers: &[sts2_core::Listener<'_, V, P>],
    model: &Model<V>,
) -> i32 {
    match monster {
        __c0 if __c0 == V::Monster::GAS_BOMB || __c0 == V::Monster::WATERFALL_GIANT => {
            underdocks::computed_damage::<V>(monster, move_id, ascension, model)
        }
        _ => glory::computed_damage::<V, _>(monster, move_id, ascension, powers),
    }
}

/// A move's hit count that the monster works out from what its own model
/// saved.
///
/// # Panics
///
/// A move nobody has ported.
pub(crate) fn computed_repeats<V: crate::Build>(
    monster: V::Monster,
    move_id: V::MoveId,
    model: &Model<V>,
) -> u32 {
    glory::computed_repeats::<V>(monster, move_id, model)
}

/// Which branch a monster's conditional state takes.
///
/// `ConditionalBranchState`, whose predicates read fields an encounter set on
/// the monster when it built the roster. No table holds those, so each is
/// ported beside the monster whose graph asks.
///
/// # Panics
///
/// A conditional nobody has ported.
pub(crate) fn conditional<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    monster: V::Monster,
    at: u8,
    spawn: MonsterSpawn<V::Encounter>,
    powers: &[sts2_core::Listener<'_, V, P>],
    model: &Model<V>,
) -> Result<u8, ContentError> {
    match monster {
        __c0 if __c0 == V::Monster::KNOWLEDGE_DEMON => hive::knowledge_demon_branch::<V>(model, at),
        __c0 if __c0 == V::Monster::BOWLBUG_ROCK
            || __c0 == V::Monster::SLUMBERING_BEETLE
            || __c0 == V::Monster::OVICOPTER =>
        {
            hive::conditional::<V, _>(monster, at, spawn, powers)
        }
        __c0 if __c0 == V::Monster::EXOSKELETON
            || __c0 == V::Monster::MYTE
            || __c0 == V::Monster::PHANTASMAL_GARDENER =>
        {
            opens_by_slot::<V>(monster, at, spawn)
        }
        __c0 if __c0 == V::Monster::TOADPOLE => underdocks::conditional::<V>(monster, at, spawn),
        __c0 if __c0 == V::Monster::LAGAVULIN_MATRIARCH => {
            underdocks::matriarch_branch::<V, _>(at, powers)
        }
        __c0 if __c0 == V::Monster::LIVING_SHIELD
            || __c0 == V::Monster::FROG_KNIGHT
            || __c0 == V::Monster::FABRICATOR
            || __c0 == V::Monster::TEST_SUBJECT
            || __c0 == V::Monster::QUEEN =>
        {
            glory::conditional::<V>(monster, at, spawn, model)
        }
        _ => overgrowth::conditional::<V>(monster, at, spawn),
    }
}

/// An `INIT_MOVE` that reads the monster's `SlotName`. The Exoskeleton's, the
/// Myte's and the Phantasmal Gardener's encounters name their slots `first`
/// onwards in roster order.
/// The state adds a branch for each slot in that order.
fn opens_by_slot<V: crate::Build>(
    monster: V::Monster,
    at: u8,
    spawn: MonsterSpawn<V::Encounter>,
) -> Result<u8, ContentError> {
    if !matches!((monster, spawn.encounter), ( __c0, __c1 ) if (__c0 == V::Monster::EXOSKELETON && (__c1 == V::Encounter::EXOSKELETONS_WEAK || __c1 == V::Encounter::EXOSKELETONS_NORMAL)) || (__c0 == V::Monster::MYTE && __c1 == V::Encounter::MYTES_NORMAL) || (__c0 == V::Monster::PHANTASMAL_GARDENER && __c1 == V::Encounter::PHANTASMAL_GARDENERS_ELITE))
    {
        unimplemented!("a {monster:?} spawned {spawn:?}");
    }
    let invalid = ContentError::InvalidMonsterState { index: at };
    match V::monster_conditional(monster, at) {
        Some(branches) => branches
            .get(usize::from(spawn.position.get()))
            .copied()
            .ok_or(invalid),
        _ => Err(invalid),
    }
}

/// The weight of the branch at `at` in a random state, where its
/// `weightLambda` reads the fight.
///
/// # Panics
///
/// A weight nobody has ported.
pub(crate) fn weight<V: crate::Build>(
    monster: V::Monster,
    at: usize,
    spawn: MonsterSpawn<V::Encounter>,
    model: &Model<V>,
    advertised: impl Fn(V::MoveId) -> bool,
) -> f32 {
    match monster {
        __c0 if __c0 == V::Monster::TWO_TAILED_RAT => {
            underdocks::rat_weight::<V>(at, spawn, model, advertised)
        }
        monster => unimplemented!("the weight of {monster:?}'s branch {at}"),
    }
}

/// Where a named move sits in a monster's own state list.
///
/// A conditional's branch is named by the move it leads to rather than by a
/// number. A graph adds its branches from the same fields a condition reads.
/// So a dumped list is the one the dump's own monster had, and a branch some
/// other one takes can be missing.
pub(crate) fn state_of<V: crate::Build>(
    monster: V::Monster,
    wanted: V::MoveId,
    at: u8,
) -> Result<u8, ContentError> {
    V::monster_state_of(monster, wanted, at)
}

/// Deliver a notification to one monster's own model.
///
/// The fight asks only a monster whose class overrides the hook.
///
/// # Panics
///
/// An override nobody has ported.
pub(crate) fn event_hook<V: crate::Build>(
    monster: V::Monster,
    model: &mut Model<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (monster, hook) {
        // A Decimillipede segment swaps its texture as it dies, the Vantom
        // moves the music on, and the Kin Priest talks and moves the music
        // on as its followers die. None of them touches the fight.
        (__c0, EventHook::AfterDeath { .. })
            if (__c0 == V::Monster::DECIMILLIPEDE_SEGMENT_FRONT
                || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_MIDDLE
                || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_BACK
                || __c0 == V::Monster::VANTOM
                || __c0 == V::Monster::KIN_PRIEST) => {}
        // The Kaiser Crab's arms move its background as their health falls,
        // and move the music on as one dies.
        (__c0, EventHook::CurrentHpChanged { .. } | EventHook::BeforeDeath { .. })
            if (__c0 == V::Monster::CRUSHER || __c0 == V::Monster::ROCKET) => {}
        (
            __c0,
            EventHook::AfterDeath {
                monster: Some(__c1),
                ..
            },
        ) if (__c0 == V::Monster::QUEEN && __c1 == V::Monster::TORCH_HEAD_AMALGAM) => {
            glory::amalgam_died::<V>(model, asked)
        }
        // The Insatiable's own death does nothing but return. The Test
        // Subject's sets its colour and the music, and the Aeonglass's
        // returns. The Queen's own sets the music and the portrait. The Soul
        // Fysh's sets the music, and the Waterfall Giant's stops its sound.
        (__c0, EventHook::AfterDeath { .. })
            if (__c0 == V::Monster::THE_INSATIABLE
                || __c0 == V::Monster::TEST_SUBJECT
                || __c0 == V::Monster::AEONGLASS
                || __c0 == V::Monster::QUEEN
                || __c0 == V::Monster::SOUL_FYSH
                || __c0 == V::Monster::WATERFALL_GIANT) => {}
        // The Lagavulin Matriarch opens its shell's eyes once a hit leaves it
        // at half health or below, and closes them as it dies. Both are on
        // screen alone.
        (__c0, EventHook::DamageReceived { .. } | EventHook::AfterDeath { .. })
            if __c0 == V::Monster::LAGAVULIN_MATRIARCH => {}
        (__c0, EventHook::CardGeneratedForCombat { at, card, .. })
            if __c0 == V::Monster::AEONGLASS =>
        {
            glory::wither_generated::<V>(model, at, card, asked)
        }
        (monster, hook) => unimplemented!("what {monster:?} does for {hook:?}"),
    }
}

/// Perform one of a monster's moves.
///
/// # Panics
///
/// A monster nobody has ported.
#[tracing::instrument(level = "trace", skip_all, fields(monster = ?monster, move_id = ?move_id))]
pub(crate) fn perform<V: crate::Build, F: Fight<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    fight: &mut F,
) -> Played<V> {
    perform_from::<V, _>(monster, move_id, 0, fight)
}

/// A move whose attack stood still on a question the player had to answer,
/// carried on from where it stopped.
pub(crate) fn resume_move<V: crate::Build, F: Fight<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    at: Resume,
    fight: &mut F,
) -> Played<V> {
    let Resume::Step(step) = at else {
        unreachable!("{at:?} resumes a choice, not a move")
    };
    perform_from::<V, _>(monster, move_id, step, fight)
}

/// A move from `step`, which stands still, naming the step it goes on from,
/// where its attack made the player answer something.
fn perform_from<V: crate::Build, F: Fight<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    step: u8,
    fight: &mut F,
) -> Played<V> {
    // Two states no table names. `Creature.Stun` builds one and
    // `IllusionPower` builds the other, and what each does belongs to
    // whoever built it rather than to the monster standing on it.
    match move_id {
        // `SlumberPower` stuns the beetle with `WakeUpMove` as the stun's own
        // move, which takes its Plating off. Whistle's plain stun does
        // nothing, and lands only on a beetle still asleep or already awake:
        // one with Slumber, or with no Plating left.
        __c0 if __c0 == V::MoveId::STUNNED && (monster == V::Monster::SLUMBERING_BEETLE) => {
            use sts2_core::CreatureView as _;
            let actor = fight.actor();
            let asleep = fight.creatures().any(|creature| {
                creature.id() == actor && creature.power(V::Power::SLUMBER_POWER).is_some()
            });
            if !asleep {
                fight.remove_power(actor, V::Power::PLATING_POWER);
            }
            return Played::Done;
        }
        __c0 if __c0 == V::MoveId::STUNNED => return Played::Done,
        __c0 if __c0 == V::MoveId::REVIVE_MOVE => {
            revive::<V, _>(fight);
            return Played::Done;
        }
        _ => {}
    }
    if let (__c0, __c1) = (monster, move_id)
        && __c0 == V::Monster::KNOWLEDGE_DEMON
        && (__c1 == V::MoveId::CURSE_OF_KNOWLEDGE_MOVE
            || __c1 == V::MoveId::KNOWLEDGE_OVERWHELMING_MOVE
            || __c1 == V::MoveId::SLAP_MOVE
            || __c1 == V::MoveId::PONDER_MOVE)
    {
        return hive::knowledge_demon::<V, _>(move_id, step, fight);
    }
    // A move whose rule is the build's own.
    let next = match V::own_move(monster, move_id, step, fight) {
        Some(next) => next,
        None => match V::filed(monster) {
            Filed::Glory => glory::perform::<V, _>(monster, move_id, step, fight),
            Filed::Hive => hive::perform::<V, _>(monster, move_id, step, fight),
            Filed::Overgrowth => overgrowth::perform::<V, _>(monster, move_id, step, fight),
            Filed::Shared => shared::perform::<V, _>(monster, move_id, step, fight),
            Filed::Summoned => summoned::perform::<V, _>(monster, move_id, step, fight),
            Filed::Underdocks => underdocks::perform::<V, _>(monster, move_id, step, fight),
        },
    };
    match next {
        Some(step) => Played::Waiting {
            at: Resume::Step(step).into(),
        },
        None => Played::Done,
    }
}

/// A move's opening attack, at step 0. True where the attack made the player
/// answer something, and the move stands still until step 1.
///
/// The attack is the one command a move can stand still on. Every other
/// command takes a `ThrowingPlayerChoiceContext`, which throws on a question.
/// Each move ported by hand attacks first or last, so none needs a step past
/// 1. The board panics on a command that runs on past an attack that asked.
fn attack_first<V: crate::Build, F: Fight<V>>(fight: &mut F, step: u8) -> bool {
    if step == 0 {
        fight.attack_opponents();
        return fight.waiting();
    }
    false
}

/// What a monster's own model saves between its moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model<V: crate::Build> {
    /// A monster whose moves save nothing.
    Stateless,
    /// `KnowledgeDemon.CurseOfKnowledgeCounter`: the curses it has cast. Its
    /// `IsBurnt` picks animations alone.
    KnowledgeDemon { curses: u8 },
    /// `FrogKnight.HasBeetleCharged`.
    FrogKnight { beetle_charged: bool },
    /// `Axebot._stockOverrideAmount`, which a respawn writes.
    Axebot { stock: Option<u8> },
    /// `Fabricator._lastSpawned`.
    Fabricator { last: Option<V::Monster> },
    /// `TestSubject.Respawns` and `ExtraMultiClawCount`.
    TestSubject { respawns: u8, extra_claws: u16 },
    /// `Aeonglass.AdditionalStrength` and `WitherUpgradeCount`.
    Aeonglass { strength: u8, wither_upgrades: u8 },
    /// `Queen.HasAmalgamDied`.
    Queen { amalgam_died: bool },
    /// `WaterfallGiant.CurrentPressureGunDamage`, as the times its Pressure
    /// Gun has raised it, and `SteamEruptionDamage`.
    WaterfallGiant { gun_raises: u8, eruption: Number },
    /// `TwoTailedRat.TurnsUntilSummonable`, held no lower than zero since
    /// only above zero bars a summon, and `CallForBackupCount`.
    TwoTailedRat { summonable_in: u8, backups: u8 },
    /// `PunchConstruct.StartingHpReduction`, which the Punch-Off event's
    /// encounter writes.
    PunchConstruct { hp_reduction: u8 },
}

impl<V: crate::Build> sts2_core::MonsterModel<V> for Model<V> {
    fn new(monster: V::Monster) -> Self {
        match monster {
            __c0 if __c0 == V::Monster::KNOWLEDGE_DEMON => Self::KnowledgeDemon { curses: 0 },
            __c0 if __c0 == V::Monster::FROG_KNIGHT => Self::FrogKnight {
                beetle_charged: false,
            },
            __c0 if __c0 == V::Monster::AXEBOT => Self::Axebot { stock: None },
            __c0 if __c0 == V::Monster::FABRICATOR => Self::Fabricator { last: None },
            __c0 if __c0 == V::Monster::TEST_SUBJECT => Self::TestSubject {
                respawns: 0,
                extra_claws: 0,
            },
            __c0 if __c0 == V::Monster::AEONGLASS => Self::Aeonglass {
                strength: 0,
                wither_upgrades: 0,
            },
            __c0 if __c0 == V::Monster::QUEEN => Self::Queen {
                amalgam_died: false,
            },
            __c0 if __c0 == V::Monster::WATERFALL_GIANT => Self::WaterfallGiant {
                gun_raises: 0,
                eruption: 0,
            },
            __c0 if __c0 == V::Monster::TWO_TAILED_RAT => Self::TwoTailedRat {
                summonable_in: underdocks::RAT_SUMMONABLE_IN,
                backups: 0,
            },
            __c0 if __c0 == V::Monster::PUNCH_CONSTRUCT => Self::PunchConstruct { hp_reduction: 0 },
            _ => Self::Stateless,
        }
    }

    fn written(monster: V::Monster, written: Number) -> Self {
        match monster {
            __c0 if __c0 == V::Monster::AXEBOT => Self::Axebot {
                stock: Some(u8::try_from(written).expect("a stock fits a byte")),
            },
            __c0 if __c0 == V::Monster::PUNCH_CONSTRUCT => Self::PunchConstruct {
                hp_reduction: u8::try_from(written).expect("a reduction fits a byte"),
            },
            monster => unimplemented!("a power writing {written} onto a new {monster:?}"),
        }
    }
}
