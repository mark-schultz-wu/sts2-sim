//! The monsters no act fields, which an event or another monster brings.

use sts2_core::PowerAmount;

use super::super::super::generated::monsters::Monster;
use super::super::super::generated::powers::Power;
use super::Number;

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
