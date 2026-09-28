//! The third act's monsters.

use sts2_core::{Fight, PowerAmount};

use super::super::super::generated::monster_moves;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::super::super::generated::powers::Power;
use super::{Number, V};

/// Powers a Glory monster applies to itself as it enters combat.
pub(super) fn starting_powers(monster: Monster) -> Option<Vec<(Power, PowerAmount<Number>)>> {
    Some(match monster {
        // `_buffAmt`, which is also Paper Cuts'.
        Monster::ScrollOfBiting => vec![(Power::PaperCutsPower, PowerAmount::Counted(2))],
        Monster::LivingShield => vec![(Power::RampartPower, PowerAmount::Counted(25))],
        _ => return None,
    })
}

/// Which move a Glory monster selects from a conditional state.
pub(super) fn conditional(
    monster: Monster,
    at: u8,
    spawn: sts2_core::MonsterSpawn<super::super::super::generated::encounters::Encounter>,
) -> Result<u8, sts2_core::ContentError> {
    let wanted = match monster {
        // `SHIELD_SLAM_BRANCH`: Shield Slam again while an ally stands, and
        // Smash once none does.
        Monster::LivingShield => match spawn.standing > 1 {
            true => MoveId::ShieldSlamMove,
            false => MoveId::SmashMove,
        },
        monster => unimplemented!("what {monster:?} asks the fight at state {at}"),
    };
    super::state_of(monster, wanted, at)
}

/// `DevotedSculptor`'s `_ritualGain`.
const RITUAL: Number = 9;

/// Perform one of the Glory monsters' moves, where the generator left it to
/// a hand port.
pub(super) fn perform<F: Fight<V>>(monster: Monster, move_id: MoveId, fight: &mut F) {
    match (monster, move_id) {
        // `ForbiddenIncantationMove`: Ritual on itself, from nobody.
        (Monster::DevotedSculptor, MoveId::ForbiddenIncantationMove) => {
            let actor = fight.actor();
            fight.apply_power_from(
                actor,
                Power::RitualPower,
                PowerAmount::Counted(RITUAL),
                None,
            );
        }
        _ => monster_moves::perform(monster, move_id, fight),
    }
}
