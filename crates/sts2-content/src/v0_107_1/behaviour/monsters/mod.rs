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

mod overgrowth;

type V = V0_107_1;

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
    match monster {
        Monster::Nibbit => overgrowth::nibbit_opens(at, spawn),
        monster => unimplemented!("what {monster:?} asks the fight at state {at}"),
    }
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
        Monster::LeafSlimeM => overgrowth::leaf_slime_m(move_id, fight),
        Monster::ShrinkerBeetle => overgrowth::shrinker_beetle(move_id, fight),
        Monster::LeafSlimeS => overgrowth::leaf_slime_s(move_id, fight),
        Monster::TwigSlimeM => overgrowth::twig_slime_m(move_id, fight),
        Monster::TwigSlimeS => overgrowth::twig_slime_s(move_id, fight),
        Monster::Nibbit => overgrowth::nibbit(move_id, fight),
        monster => unimplemented!("what {monster:?} does when it moves"),
    }
}
