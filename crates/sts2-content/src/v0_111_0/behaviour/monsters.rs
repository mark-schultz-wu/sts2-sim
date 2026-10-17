//! The monster moves whose rules this build ports for itself, which the
//! shared monsters hand over through [`crate::build::OwnRules`].

use sts2_core::Fight;

use super::super::V0_111_0 as V;
use super::super::generated::monsters::{Monster, MoveId};

/// A move of this build's own, performed: the step to go on from, or nothing
/// for a shared move.
pub(crate) fn own_move<F: Fight<V>>(
    monster: Monster,
    move_id: MoveId,
    _step: u8,
    fight: &mut F,
) -> Option<Option<u8>> {
    match (monster, move_id) {
        // `Hatch` draws `NextInt(HatchlingMinHp, HatchlingMaxHp + 1)`, so the
        // top of the range can come up.
        (Monster::ToughEgg, MoveId::HatchMove) => {
            crate::behaviour::monsters::hive::hatch::<V, _>(fight, |fight, min, max| {
                fight.niche_int(min, max + 1)
            });
            Some(None)
        }
        _ => None,
    }
}
