//! The monsters no act fields, which an event or another monster brings.

use sts2_core::{Fight, PowerAmount};

use crate::behaviour::monsters::Number;
use crate::ids::{MonsterIds as _, MoveIds as _, PowerIds as _};

/// The Mysterious Knight's `AfterAddedToRoom`: `PowerCmd.Apply<StrengthPower>`
/// then `PowerCmd.Apply<PlatingPower>`, six of each, on itself.
const KNIGHT: Number = 6;

/// Powers one of these monsters applies to itself as it enters combat.
pub(super) fn starting_powers<V: crate::Build>(
    monster: V::Monster,
) -> Option<Vec<(V::Power, PowerAmount<Number>)>> {
    Some(match monster {
        __c0 if __c0 == V::Monster::MYSTERIOUS_KNIGHT => vec![
            (V::Power::STRENGTH_POWER, PowerAmount::Counted(KNIGHT)),
            (V::Power::PLATING_POWER, PowerAmount::Counted(KNIGHT)),
        ],
        // Battleworn Dummy's three settings, each with three turns to fall.
        __c0 if __c0 == V::Monster::BATTLE_FRIEND_V1
            || __c0 == V::Monster::BATTLE_FRIEND_V2
            || __c0 == V::Monster::BATTLE_FRIEND_V3 =>
        {
            vec![(
                V::Power::BATTLEWORN_DUMMY_TIME_LIMIT_POWER,
                PowerAmount::Counted(3),
            )]
        }
        _ => return None,
    })
}

/// Perform one of these monsters' moves, where the generator left it to a
/// hand port.
pub(super) fn perform<V: crate::Build, F: Fight<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    step: u8,
    fight: &mut F,
) -> Option<u8> {
    match (monster, move_id) {
        // `NOTHING_MOVE`, whose lambda is `Task.CompletedTask`.
        (__c0, __c1)
            if ((__c0 == V::Monster::BATTLE_FRIEND_V1
                || __c0 == V::Monster::BATTLE_FRIEND_V2
                || __c0 == V::Monster::BATTLE_FRIEND_V3)
                && __c1 == V::MoveId::NOTHING_MOVE) =>
        {
            None
        }
        (monster, move_id) => V::generated_move(monster, move_id, step, fight),
    }
}
