//! The monsters met in more than one act.

use sts2_core::{Fight, PowerAmount};

use crate::behaviour::monsters::Number;
use crate::ids::{MonsterIds as _, MoveIds as _, PowerIds as _};

/// What the Cubex Construct's Charge Up adds to it, a constant in the move.
const CHARGE_UP_STRENGTH: Number = 2;

/// Powers one of these monsters applies to itself as it enters combat.
///
/// The Cubex Construct gains block first, which `CreatureCmd.GainBlock`
/// refuses while the fight is not yet in progress.
pub(super) fn starting_powers<V: crate::Build>(
    monster: V::Monster,
) -> Option<Vec<(V::Power, PowerAmount<Number>)>> {
    Some(match monster {
        __c0 if __c0 == V::Monster::CUBEX_CONSTRUCT => {
            vec![(V::Power::ARTIFACT_POWER, PowerAmount::Counted(1))]
        }
        _ => return None,
    })
}

/// Perform one of these monsters' moves, where the generator left it to a
/// hand port.
pub(super) fn perform<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    fight: &mut K::Fight<'a>,
) -> crate::behaviour::Played<V, K> {
    match (monster, move_id) {
        // `IsBurrowed` and `IsCharging`, which it sets first, pick its
        // animation.
        (__c0, __c1)
            if (__c0 == V::Monster::CUBEX_CONSTRUCT && __c1 == V::MoveId::CHARGE_UP_MOVE) =>
        {
            fight.apply_to_actor(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(CHARGE_UP_STRENGTH),
            );
        }
        (monster, move_id) => return V::generated_move::<K>(monster, move_id, fight),
    }
    crate::behaviour::Played::Done
}
