//! The monster moves whose rules this build ports for itself, which the
//! shared monsters hand over through [`crate::build::OwnRules`].

use sts2_core::{Fight, FightKind};

use super::super::V0_111_0 as V;
use super::super::generated::monsters::{Monster, MoveId};

/// A move of this build's own, performed, or nothing for a shared move.
pub(crate) fn own_move<'a, K: FightKind<V>>(
    monster: Monster,
    move_id: MoveId,
    fight: &mut K::Fight<'a>,
) -> Option<crate::behaviour::Played<V, K>> {
    match (monster, move_id) {
        // `Hatch` draws `NextInt(HatchlingMinHp, HatchlingMaxHp + 1)`, so the
        // top of the range can come up.
        (Monster::ToughEgg, MoveId::HatchMove) => {
            crate::behaviour::monsters::hive::hatch::<V, _>(fight, |fight, min, max| {
                fight.niche_int(min, max + 1)
            });
            Some(crate::behaviour::Played::Done)
        }
        (Monster::MechaKnight, MoveId::FlamethrowerMove) => Some(start::<K>(move_id, fight)),
        _ => None,
    }
}

/// A move of this build's own that can stop on a question. It attacks, and
/// does the rest once any question the attack left is answered. Each step is
/// lent the fight, and after a question the answer.
#[diapause::coroutine(
    yield = crate::behaviour::paused::Ask<V>,
    resume = for<'f, 'a> crate::behaviour::paused::Lent<'f, 'a, V, K>
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<K: FightKind<V>>(move_id: MoveId) {
    match move_id {
        // The Mecha Knight's `FlamethrowerMove`: its hit, then four Burns
        // into each player's hand. v0.107.1's added the Burns alone.
        MoveId::FlamethrowerMove => {
            resumed!().fight.attack_opponents();
            if resumed!().fight.waiting() {
                yield_!(crate::behaviour::paused::Ask::Waiting);
            }
            let fight = &mut *resumed!().fight;
            for player in fight.players() {
                fight.for_player(player, |fight| {
                    fight.add_generated_cards(
                        sts2_core::CardIdent::new(crate::v0_111_0::cards::Cards::Status(
                            crate::v0_111_0::cards::StatusCard::Burn,
                        )),
                        sts2_core::Pile::Hand,
                        4,
                    );
                });
            }
        }
        other => unreachable!("{other:?} never stops on a question"),
    }
}

/// A move of [`paused`], from its start.
fn start<'a, K: FightKind<V>>(
    move_id: MoveId,
    fight: &mut K::Fight<'a>,
) -> crate::behaviour::Played<V, K> {
    let mut play = paused::<K>(move_id);
    let state = play.start_with(crate::behaviour::paused::Lent { fight, taken: None });
    settle::<K>(play, state)
}

/// A paused move of [`paused`], with the answer it waited for.
pub(crate) fn resume<'a, K: FightKind<V>>(
    fight: &mut K::Fight<'a>,
    mut play: paused::State<K>,
    taken: sts2_core::Taken<V>,
) -> crate::behaviour::Played<V, K> {
    let state = diapause::Coroutine::resume(
        &mut play,
        crate::behaviour::paused::Lent {
            fight,
            taken: Some(taken),
        },
    );
    settle::<K>(play, state)
}

/// How far a step of `play` got.
fn settle<K: FightKind<V>>(
    play: paused::State<K>,
    state: diapause::CoroutineState<crate::behaviour::paused::Ask<V>, ()>,
) -> crate::behaviour::Played<V, K> {
    match state {
        diapause::CoroutineState::Complete(()) => crate::behaviour::Played::Done,
        diapause::CoroutineState::Yielded(ask) => ask.at(crate::behaviour::Resume::Build(
            super::Paused::OwnMoves(play),
        )),
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
