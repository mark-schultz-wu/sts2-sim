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

use super::super::V0_107_1;
use super::super::generated::encounters::Encounter;
use super::super::generated::monster_moves::{self, Filed};
use super::super::generated::monsters::{Monster, MoveId, State};
use super::super::generated::powers::Power;

/// What a move's own numbers are held as.
pub(crate) type Number = <V0_107_1 as sts2_core::ContentVersion>::Number;

mod glory;
mod hive;
mod overgrowth;
mod shared;
mod summoned;

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
pub(crate) fn resume<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: Option<sts2_core::CardIdent<V>>,
) -> Played<V> {
    hive::resume(fight, at, chosen)
}

type V = V0_107_1;

/// `IllusionPower.ReviveMove`: the illusion comes back whole.
///
/// `CreatureCmd.Heal(Owner, MaxHp - CurrentHp)`, which is what takes a dead
/// one back over zero.
fn revive<F: Fight<V>>(fight: &mut F) {
    use sts2_core::CreatureView as _;

    let actor = fight.actor();
    let Some(missing) = fight
        .creatures()
        .find(|creature| creature.id() == actor)
        .map(|creature| creature.max_hp() - creature.hp())
    else {
        return;
    };
    fight.write_power(actor, Power::IllusionPower, |state| {
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
pub(crate) fn starting_powers(
    monster: Monster,
    ascension: <V0_107_1 as sts2_core::ContentVersion>::AscensionLevel,
    side: super::super::generated::runstate::CombatSide,
    model: &Model,
) -> Vec<(
    super::super::generated::powers::Power,
    sts2_core::PowerAmount<Number>,
)> {
    use sts2_core::MonsterTable as _;

    match overgrowth::starting_powers(monster, ascension)
        .or_else(|| hive::starting_powers(monster, ascension, side))
        .or_else(|| glory::starting_powers(monster, ascension, model))
        .or_else(|| shared::starting_powers(monster))
        .or_else(|| summoned::starting_powers(monster))
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
pub(crate) fn starting_opponent_powers(
    monster: Monster,
) -> Vec<(
    super::super::generated::powers::Power,
    sts2_core::PowerAmount<Number>,
)> {
    match monster {
        Monster::Rocket => vec![(
            super::super::generated::powers::Power::SurroundedPower,
            sts2_core::PowerAmount::Counted(1),
        )],
        _ => Vec::new(),
    }
}

/// What `AfterAddedToRoom` writes over the health a monster was built with.
///
/// Filed by act beside the moves, the way the starting powers are.
pub(crate) fn health_after_added(
    monster: Monster,
    ascension: <V0_107_1 as sts2_core::ContentVersion>::AscensionLevel,
    rolled: <V0_107_1 as sts2_core::ContentVersion>::Health,
    teammates: &[<V0_107_1 as sts2_core::ContentVersion>::Health],
) -> Option<<V0_107_1 as sts2_core::ContentVersion>::Health> {
    hive::health_after_added(monster, ascension, rolled, teammates)
}

/// A move's attack damage that the monster works out from the fight, from
/// the powers on its own creature as the move rolls.
///
/// # Panics
///
/// A move nobody has ported.
pub(crate) fn computed_damage<P: sts2_core::Printed<V> + ?Sized>(
    monster: Monster,
    move_id: MoveId,
    ascension: u8,
    powers: &[sts2_core::Listener<'_, V, P>],
) -> i32 {
    glory::computed_damage(monster, move_id, ascension, powers)
}

/// A move's hit count that the monster works out from what its own model
/// saved.
///
/// # Panics
///
/// A move nobody has ported.
pub(crate) fn computed_repeats(monster: Monster, move_id: MoveId, model: &Model) -> u32 {
    glory::computed_repeats(monster, move_id, model)
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
pub(crate) fn conditional<P: sts2_core::Printed<V> + ?Sized>(
    monster: Monster,
    at: u8,
    spawn: MonsterSpawn<Encounter>,
    powers: &[sts2_core::Listener<'_, V, P>],
    model: &Model,
) -> Result<u8, ContentError> {
    match monster {
        Monster::KnowledgeDemon => hive::knowledge_demon_branch(model, at),
        Monster::BowlbugRock | Monster::SlumberingBeetle | Monster::Ovicopter => {
            hive::conditional(monster, at, spawn, powers)
        }
        Monster::Exoskeleton | Monster::Myte => opens_by_slot(monster, at, spawn),
        Monster::LivingShield
        | Monster::FrogKnight
        | Monster::Fabricator
        | Monster::TestSubject
        | Monster::Queen => glory::conditional(monster, at, spawn, model),
        _ => overgrowth::conditional(monster, at, spawn),
    }
}

/// An `INIT_MOVE` that reads the monster's `SlotName`. The Exoskeleton's and
/// the Myte's encounters name their slots `first` onwards in roster order.
/// The state adds a branch for each slot in that order.
fn opens_by_slot(
    monster: Monster,
    at: u8,
    spawn: MonsterSpawn<Encounter>,
) -> Result<u8, ContentError> {
    if !matches!(
        (monster, spawn.encounter),
        (
            Monster::Exoskeleton,
            Encounter::ExoskeletonsWeak | Encounter::ExoskeletonsNormal
        ) | (Monster::Myte, Encounter::MytesNormal)
    ) {
        unimplemented!("a {monster:?} spawned {spawn:?}");
    }
    let invalid = ContentError::InvalidMonsterState { index: at };
    match sts2_core::Table::data(monster).states.get(usize::from(at)) {
        Some(State::Conditional(branches)) => branches
            .get(usize::from(spawn.position.get()))
            .copied()
            .ok_or(invalid),
        _ => Err(invalid),
    }
}

/// Where a named move sits in a monster's own state list.
///
/// A conditional's branch is named by the move it leads to rather than by a
/// number. A graph adds its branches from the same fields a condition reads.
/// So a dumped list is the one the dump's own monster had, and a branch some
/// other one takes can be missing.
pub(crate) fn state_of(monster: Monster, wanted: MoveId, at: u8) -> Result<u8, ContentError> {
    sts2_core::Table::data(monster)
        .states
        .iter()
        .position(|state| matches!(state, State::Move(held) if held.id == wanted))
        .and_then(|found| u8::try_from(found).ok())
        .ok_or(ContentError::InvalidMonsterState { index: at })
}

/// Deliver a notification to one monster's own model.
///
/// The fight asks only a monster whose class overrides the hook.
///
/// # Panics
///
/// An override nobody has ported.
pub(crate) fn event_hook(
    monster: Monster,
    model: &mut Model,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (monster, hook) {
        // A Decimillipede segment swaps its texture as it dies, the Vantom
        // moves the music on, and the Kin Priest talks and moves the music
        // on as its followers die. None of them touches the fight.
        (
            Monster::DecimillipedeSegmentFront
            | Monster::DecimillipedeSegmentMiddle
            | Monster::DecimillipedeSegmentBack
            | Monster::Vantom
            | Monster::KinPriest,
            EventHook::AfterDeath { .. },
        ) => {}
        // The Kaiser Crab's arms move its background as their health falls,
        // and move the music on as one dies.
        (
            Monster::Crusher | Monster::Rocket,
            EventHook::CurrentHpChanged { .. } | EventHook::BeforeDeath { .. },
        ) => {}
        (
            Monster::Queen,
            EventHook::AfterDeath {
                monster: Some(Monster::TorchHeadAmalgam),
                ..
            },
        ) => glory::amalgam_died(model, asked),
        // The Insatiable's own death does nothing but return. The Test
        // Subject's sets its colour and the music, and the Aeonglass's
        // returns. The Queen's own sets the music and the portrait.
        (
            Monster::TheInsatiable | Monster::TestSubject | Monster::Aeonglass | Monster::Queen,
            EventHook::AfterDeath { .. },
        ) => {}
        (Monster::Aeonglass, EventHook::CardGeneratedForCombat { at, card, .. }) => {
            glory::wither_generated(model, at, card, asked)
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
pub(crate) fn perform<F: Fight<V>>(
    monster: Monster,
    move_id: MoveId,
    model: &mut Model,
    fight: &mut F,
) -> Played<V> {
    perform_from(monster, move_id, 0, model, fight)
}

/// A move whose attack stood still on a question the player had to answer,
/// carried on from where it stopped.
pub(crate) fn resume_move<F: Fight<V>>(
    monster: Monster,
    move_id: MoveId,
    at: Resume,
    model: &mut Model,
    fight: &mut F,
) -> Played<V> {
    let Resume::Step(step) = at else {
        unreachable!("{at:?} resumes a choice, not a move")
    };
    perform_from(monster, move_id, step, model, fight)
}

/// A move from `step`, which stands still, naming the step it goes on from,
/// where its attack made the player answer something.
fn perform_from<F: Fight<V>>(
    monster: Monster,
    move_id: MoveId,
    step: u8,
    model: &mut Model,
    fight: &mut F,
) -> Played<V> {
    // Two states no table names. `Creature.Stun` builds one and
    // `IllusionPower` builds the other, and what each does belongs to
    // whoever built it rather than to the monster standing on it.
    match move_id {
        MoveId::Stunned => return Played::Done,
        MoveId::ReviveMove => {
            revive(fight);
            return Played::Done;
        }
        _ => {}
    }
    if monster == Monster::KnowledgeDemon {
        return hive::knowledge_demon(move_id, step, model, fight);
    }
    let next = match monster_moves::filed(monster) {
        Filed::Glory => glory::perform(monster, move_id, step, model, fight),
        Filed::Hive => hive::perform(monster, move_id, step, fight),
        Filed::Overgrowth => overgrowth::perform(monster, move_id, step, fight),
        Filed::Shared => shared::perform(monster, move_id, step, fight),
        Filed::Summoned => summoned::perform(monster, move_id, step, fight),
        Filed::Underdocks => monster_moves::underdocks::perform(monster, move_id, step, fight),
    };
    match next {
        Some(step) => Played::Waiting {
            at: super::Resume::Monster(Resume::Step(step)),
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
/// 1.
fn attack_first<F: Fight<V>>(fight: &mut F, step: u8) -> bool {
    if step == 0 {
        fight.attack_opponents();
        return fight.waiting();
    }
    false
}

/// What a monster's own model saves between its moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
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
    Fabricator { last: Option<Monster> },
    /// `TestSubject.Respawns` and `ExtraMultiClawCount`.
    TestSubject { respawns: u8, extra_claws: u8 },
    /// `Aeonglass.AdditionalStrength` and `WitherUpgradeCount`.
    Aeonglass { strength: u8, wither_upgrades: u8 },
    /// `Queen.HasAmalgamDied`.
    Queen { amalgam_died: bool },
}

impl sts2_core::MonsterModel<V> for Model {
    fn new(monster: Monster) -> Self {
        match monster {
            Monster::KnowledgeDemon => Self::KnowledgeDemon { curses: 0 },
            Monster::FrogKnight => Self::FrogKnight {
                beetle_charged: false,
            },
            Monster::Axebot => Self::Axebot { stock: None },
            Monster::Fabricator => Self::Fabricator { last: None },
            Monster::TestSubject => Self::TestSubject {
                respawns: 0,
                extra_claws: 0,
            },
            Monster::Aeonglass => Self::Aeonglass {
                strength: 0,
                wither_upgrades: 0,
            },
            Monster::Queen => Self::Queen {
                amalgam_died: false,
            },
            _ => Self::Stateless,
        }
    }

    fn written(monster: Monster, written: Number) -> Self {
        match monster {
            Monster::Axebot => Self::Axebot {
                stock: Some(u8::try_from(written).expect("a stock fits a byte")),
            },
            monster => unimplemented!("a power writing {written} onto a new {monster:?}"),
        }
    }
}
