//! The monsters no act fields, which an event or another monster brings.

use sts2_core::{Fight, PowerAmount};

use super::super::super::generated::monster_moves;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::super::super::generated::powers::Power;
use super::{Number, V};

/// The Mysterious Knight's `AfterAddedToRoom`: `PowerCmd.Apply<StrengthPower>`
/// then `PowerCmd.Apply<PlatingPower>`, six of each, on itself.
const KNIGHT: Number = 6;

/// Powers one of these monsters applies to itself as it enters combat.
pub(super) fn starting_powers(monster: Monster) -> Option<Vec<(Power, PowerAmount<Number>)>> {
    Some(match monster {
        Monster::MysteriousKnight => vec![
            (Power::StrengthPower, PowerAmount::Counted(KNIGHT)),
            (Power::PlatingPower, PowerAmount::Counted(KNIGHT)),
        ],
        // Battleworn Dummy's three settings, each with three turns to fall.
        Monster::BattleFriendV1 | Monster::BattleFriendV2 | Monster::BattleFriendV3 => {
            vec![(
                Power::BattlewornDummyTimeLimitPower,
                PowerAmount::Counted(3),
            )]
        }
        _ => return None,
    })
}

/// Perform one of these monsters' moves, where the generator left it to a
/// hand port.
pub(super) fn perform<F: Fight<V>>(
    monster: Monster,
    move_id: MoveId,
    step: u8,
    fight: &mut F,
) -> Option<u8> {
    match (monster, move_id) {
        // `NOTHING_MOVE`, whose lambda is `Task.CompletedTask`.
        (
            Monster::BattleFriendV1 | Monster::BattleFriendV2 | Monster::BattleFriendV3,
            MoveId::NothingMove,
        ) => None,
        (monster, move_id) => monster_moves::summoned::perform(monster, move_id, step, fight),
    }
}
