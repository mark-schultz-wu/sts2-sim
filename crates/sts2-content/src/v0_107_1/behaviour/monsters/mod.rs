//! What a monster's move does, ported.
//!
//! `MoveState`'s `onPerform`, one function per move. One module per act, the
//! way `generated/monsters/` is filed, so a monster's moves sit under the same
//! name as its graph. An act gets a module when its first monster is ported.
//!
//! A monster nobody has ported panics naming itself, so this tree is also the
//! list of fights the simulator can take a turn in.

use sts2_core::Fight;

use super::super::V0_107_1;
use super::super::generated::monsters::{Monster, MoveId};

mod overgrowth;

type V = V0_107_1;

/// Perform one of a monster's moves.
///
/// # Panics
///
/// A monster nobody has ported.
pub(crate) fn perform<F: Fight<V>>(monster: Monster, move_id: MoveId, fight: &mut F) {
    match monster {
        Monster::LeafSlimeM => overgrowth::leaf_slime_m(move_id, fight),
        Monster::LeafSlimeS => overgrowth::leaf_slime_s(move_id, fight),
        Monster::TwigSlimeS => overgrowth::twig_slime_s(move_id, fight),
        monster => unimplemented!("what {monster:?} does when it moves"),
    }
}
