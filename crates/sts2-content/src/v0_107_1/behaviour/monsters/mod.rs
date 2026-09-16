//! What a monster's move does, ported.
//!
//! `MoveState`'s `onPerform`, one function per move. One module per act, the
//! way `generated/monsters/` is filed, so a monster's moves sit under the same
//! name as its graph. An act gets a module when its first monster is ported.
//!
//! A monster nobody has ported panics naming itself, so this tree is also the
//! list of fights the simulator can take a turn in.

use sts2_core::{ContentError, Fight, MonsterSpawn};

use super::super::V0_107_1;
use super::super::generated::encounters::Encounter;
use super::super::generated::monsters::{Monster, MoveId, State};

/// What a move's own numbers are held as.
pub(crate) type Number = <V0_107_1 as sts2_core::ContentVersion>::Number;

mod hive;
mod overgrowth;

type V = V0_107_1;

/// Powers a monster applies to itself as it enters combat.
///
/// `AfterAddedToRoom`, where a monster that has one `PowerCmd.Apply`s on its
/// own creature. Most monsters do not override it, and enter with none.
///
/// Filed by act beside the moves, so a monster's whole port is one file.
pub(crate) fn starting_powers(
    monster: Monster,
    ascension: <V0_107_1 as sts2_core::ContentVersion>::AscensionLevel,
) -> Option<(
    super::super::generated::powers::Power,
    sts2_core::PowerAmount<Number>,
)> {
    overgrowth::starting_powers(monster, ascension)
        .or_else(|| hive::starting_powers(monster, ascension))
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
pub(crate) fn conditional(
    monster: Monster,
    at: u8,
    spawn: MonsterSpawn<Encounter>,
) -> Result<u8, ContentError> {
    overgrowth::conditional(monster, at, spawn)
}

/// Where a named move sits in a monster's own state list.
///
/// A conditional's branch is named by the move it leads to rather than by a
/// number. A graph adds its branches from the same fields a condition reads.
/// So a dumped list is the one the dump's own monster had, and a branch some
/// other one takes can be missing.
pub(super) fn state_of(monster: Monster, wanted: MoveId, at: u8) -> Result<u8, ContentError> {
    sts2_core::Table::data(monster)
        .states
        .iter()
        .position(|state| matches!(state, State::Move(held) if held.id == wanted))
        .and_then(|found| u8::try_from(found).ok())
        .ok_or(ContentError::InvalidMonsterState { index: at })
}

/// Perform one of a monster's moves.
///
/// # Panics
///
/// A monster nobody has ported.
pub(crate) fn perform<F: Fight<V>>(monster: Monster, move_id: MoveId, fight: &mut F) {
    match monster {
        Monster::ThievingHopper => hive::perform(move_id, fight),
        _ => overgrowth::perform(monster, move_id, fight),
    }
}
