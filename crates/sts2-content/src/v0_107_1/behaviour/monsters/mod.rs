//! What a monster's move does, ported.
//!
//! `MoveState`'s `onPerform`, one function per move. One module per act, the
//! way `generated/monsters/` is filed, so a monster's moves sit under the same
//! name as its graph. An act gets a module when its first monster is ported.
//!
//! A monster nobody has ported panics naming itself, so this tree is also the
//! list of fights the simulator can take a turn in.

use sts2_core::{ContentError, EventHook, Fight, MonsterSpawn};

use super::super::V0_107_1;
use super::super::generated::encounters::Encounter;
use super::super::generated::monsters::{Monster, MoveId, State};
use super::super::generated::powers::Power;

/// What a move's own numbers are held as.
pub(crate) type Number = <V0_107_1 as sts2_core::ContentVersion>::Number;

mod hive;
mod overgrowth;

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
/// own creature. Most monsters do not override it, and enter with none.
///
/// Filed by act beside the moves, so a monster's whole port is one file.
pub(crate) fn starting_powers(
    monster: Monster,
    ascension: <V0_107_1 as sts2_core::ContentVersion>::AscensionLevel,
) -> Vec<(
    super::super::generated::powers::Power,
    sts2_core::PowerAmount<Number>,
)> {
    let mut applied = overgrowth::starting_powers(monster, ascension);
    applied.extend(hive::starting_powers(monster, ascension));
    applied
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
) -> Result<u8, ContentError> {
    match monster {
        Monster::BowlbugRock | Monster::SlumberingBeetle => hive::conditional(monster, at, powers),
        _ => overgrowth::conditional(monster, at, spawn),
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
pub(crate) fn event_hook(monster: Monster, hook: EventHook<V>) {
    match (monster, hook) {
        // A Decimillipede segment swaps its texture as it dies, and the
        // Vantom moves the music on. Neither touches the fight.
        (
            Monster::DecimillipedeSegmentFront
            | Monster::DecimillipedeSegmentMiddle
            | Monster::DecimillipedeSegmentBack
            | Monster::Vantom,
            EventHook::AfterDeath { .. },
        ) => {}
        (monster, hook) => unimplemented!("what {monster:?} does for {hook:?}"),
    }
}

/// Perform one of a monster's moves.
///
/// # Panics
///
/// A monster nobody has ported.
#[tracing::instrument(level = "trace", skip_all, fields(monster = ?monster, move_id = ?move_id))]
pub(crate) fn perform<F: Fight<V>>(monster: Monster, move_id: MoveId, fight: &mut F) {
    // Two states no table names. `Creature.Stun` builds one and
    // `IllusionPower` builds the other, and what each does belongs to
    // whoever built it rather than to the monster standing on it.
    match move_id {
        MoveId::Stunned => return,
        MoveId::ReviveMove => return revive(fight),
        _ => {}
    }
    match monster {
        Monster::ThievingHopper
        | Monster::Tunneler
        | Monster::HunterKiller
        | Monster::BowlbugRock
        | Monster::BowlbugSilk
        | Monster::SlumberingBeetle
        | Monster::DecimillipedeSegmentFront
        | Monster::DecimillipedeSegmentMiddle
        | Monster::DecimillipedeSegmentBack => hive::perform(move_id, fight),
        _ => overgrowth::perform(monster, move_id, fight),
    }
}
