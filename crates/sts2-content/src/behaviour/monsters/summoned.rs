//! The monsters no act fields, which an event or another monster brings.

use sts2_core::{Fight, PowerAmount};

use crate::behaviour::monsters::Number;
use crate::ids::{MonsterIds as _, MoveIds as _, PowerIds as _};

/// The Mysterious Knight's `AfterAddedToRoom`: `PowerCmd.Apply<StrengthPower>`
/// then `PowerCmd.Apply<PlatingPower>`, six of each, on itself.
const KNIGHT: Number = 6;

/// The Frail the Fake Merchant's `ThrowRelicMove` applies.
const THROWN_FRAIL: Number = 1;

/// The Strength the Fake Merchant's `EnrageMove` gains.
const ENRAGE: Number = 2;

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
pub(super) fn perform<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    fight: &mut K::Fight<'a>,
) -> crate::behaviour::Played<V, K> {
    match (monster, move_id) {
        // `NOTHING_MOVE`, whose lambda is `Task.CompletedTask`.
        (__c0, __c1)
            if ((__c0 == V::Monster::BATTLE_FRIEND_V1
                || __c0 == V::Monster::BATTLE_FRIEND_V2
                || __c0 == V::Monster::BATTLE_FRIEND_V3)
                && __c1 == V::MoveId::NOTHING_MOVE) => {}
        // The Fake Merchant's moves each open with `ShowDialogueForMove`, a
        // line off `Rng.Chaotic` that nothing else reads.
        // `SwipeMove` and `SpewCoinsMove` are their attacks alone.
        (__c0, __c1)
            if (__c0 == V::Monster::FAKE_MERCHANT_MONSTER
                && (__c1 == V::MoveId::SWIPE_MOVE || __c1 == V::MoveId::SPEW_COINS_MOVE)) =>
        {
            fight.attack_opponents();
        }
        // `ThrowRelicMove`: its hit, then Frail at one on each target.
        (__c0, __c1)
            if (__c0 == V::Monster::FAKE_MERCHANT_MONSTER
                && __c1 == V::MoveId::THROW_RELIC_MOVE) =>
        {
            return start::<V, K>((monster, move_id), fight);
        }
        // `EnrageMove`: an animation, then Strength at two on itself.
        (__c0, __c1)
            if (__c0 == V::Monster::FAKE_MERCHANT_MONSTER && __c1 == V::MoveId::ENRAGE_MOVE) =>
        {
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(ENRAGE));
        }
        (monster, move_id) => return V::generated_move::<K>(monster, move_id, fight),
    }
    crate::behaviour::Played::Done
}

/// A move of a monster in this file that can stop on a question. It attacks,
/// and does the rest once any question the attack left is answered. Each
/// step is lent the fight, and after a question the answer.
///
/// The attack is the one command a move can stand still on. Every other
/// command takes a `ThrowingPlayerChoiceContext`, which throws on a
/// question.
#[diapause::coroutine(
    yield = crate::behaviour::paused::Ask<V>,
    resume = for<'f, 'a> crate::behaviour::paused::Lent<'f, 'a, V, K>
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: sts2_core::FightKind<V>>(item: (V::Monster, V::MoveId)) {
    let monster: V::Monster = item.0;
    let move_id: V::MoveId = item.1;
    match (monster, move_id) {
        // `ThrowRelicMove`: its hit, then Frail at one on each target.
        (__c0, __c1)
            if (__c0 == V::Monster::FAKE_MERCHANT_MONSTER
                && __c1 == V::MoveId::THROW_RELIC_MOVE) =>
        {
            resumed!().fight.attack_opponents();
            if resumed!().fight.waiting() {
                yield_!(crate::behaviour::paused::Ask::Waiting);
            }
            let fight = &mut *resumed!().fight;
            fight.apply_to_opponents(V::Power::FRAIL_POWER, PowerAmount::Counted(THROWN_FRAIL));
        }
        other => unreachable!("{other:?} never stops on a question"),
    }
}

crate::behaviour::paused::paused_play!(
    (V::Monster, V::MoveId),
    crate::behaviour::monsters::Resume::Summoned
);
