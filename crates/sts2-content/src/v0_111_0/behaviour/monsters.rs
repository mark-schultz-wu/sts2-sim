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
    step: u8,
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
        // `FlamethrowerMove`: its hit, then four Burns into the hand of the
        // one player. v0.107.1's added the Burns alone.
        (Monster::MechaKnight, MoveId::FlamethrowerMove) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(Some(1));
            }
            fight.add_generated_cards(
                sts2_core::CardIdent::new(super::super::generated::cards::Cards::Status(
                    super::super::generated::cards::StatusCard::Burn,
                )),
                sts2_core::Pile::Hand,
                4,
            );
            Some(None)
        }
        _ => None,
    }
}

/// `Axebot._respawnMaxHpBonus`: the health each respawn adds to its range.
const AXEBOT_RESPAWN_MAX_HP_BONUS: i32 = 10;

/// `Axebot._initialStockAmount`: the stock an Axebot nobody wrote on holds.
const AXEBOT_INITIAL_STOCK: u8 = 2;

/// `MinInitialHp` and `MaxInitialHp` for a monster that reads its model for
/// them.
///
/// `Axebot`: its row's range, and `RespawnMaxHpBonus` on both ends, which is
/// ten for each stock it has lost. The stock is written before the Axebot
/// joins, so its roll reads it.
pub(crate) fn own_hp_bounds(
    monster: Monster,
    model: &crate::behaviour::monsters::Model<V>,
    ascension: crate::generated::ascension::AscensionLevel,
) -> Option<sts2_core::rng::Interval<i32>> {
    match (monster, *model) {
        (Monster::Axebot, crate::behaviour::monsters::Model::Axebot { stock }) => {
            let respawns = AXEBOT_INITIAL_STOCK - stock.unwrap_or(AXEBOT_INITIAL_STOCK);
            let bonus = i32::from(respawns) * AXEBOT_RESPAWN_MAX_HP_BONUS;
            let data = sts2_core::Table::data(monster);
            Some(sts2_core::rng::Interval::new(
                i32::from(data.min_hp.at(ascension as u8)) + bonus,
                i32::from(data.max_hp.at(ascension as u8)) + bonus,
            ))
        }
        (Monster::Axebot, model) => unreachable!("an Axebot holding {model:?}"),
        _ => None,
    }
}
