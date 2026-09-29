//! The monsters met in more than one act.

use sts2_core::{Fight, PowerAmount};

use super::super::super::generated::monster_moves;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::super::super::generated::powers::Power;
use super::{Number, V};

/// What the Cubex Construct's Charge Up adds to it, a constant in the move.
const CHARGE_UP_STRENGTH: Number = 2;

/// Powers one of these monsters applies to itself as it enters combat.
///
/// The Cubex Construct gains block first, which `CreatureCmd.GainBlock`
/// refuses while the fight is not yet in progress.
pub(super) fn starting_powers(monster: Monster) -> Option<Vec<(Power, PowerAmount<Number>)>> {
    Some(match monster {
        Monster::CubexConstruct => vec![(Power::ArtifactPower, PowerAmount::Counted(1))],
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
        // `IsBurrowed` and `IsCharging`, which it sets first, pick its
        // animation.
        (Monster::CubexConstruct, MoveId::ChargeUpMove) => {
            fight.apply_to_actor(
                Power::StrengthPower,
                PowerAmount::Counted(CHARGE_UP_STRENGTH),
            );
        }
        (monster, move_id) => return monster_moves::shared::perform(monster, move_id, step, fight),
    }
    None
}
