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
        // The Fake Merchant's moves each open with `ShowDialogueForMove`, a
        // line off `Rng.Chaotic` that nothing else reads.
        // `SwipeMove` and `SpewCoinsMove` are their attacks alone.
        (__c0, __c1)
            if (__c0 == V::Monster::FAKE_MERCHANT_MONSTER
                && (__c1 == V::MoveId::SWIPE_MOVE || __c1 == V::MoveId::SPEW_COINS_MOVE)) =>
        {
            fight.attack_opponents();
            None
        }
        // `ThrowRelicMove`: its hit, then Frail at one on each target.
        (__c0, __c1)
            if (__c0 == V::Monster::FAKE_MERCHANT_MONSTER
                && __c1 == V::MoveId::THROW_RELIC_MOVE) =>
        {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(V::Power::FRAIL_POWER, PowerAmount::Counted(THROWN_FRAIL));
            None
        }
        // `EnrageMove`: an animation, then Strength at two on itself.
        (__c0, __c1)
            if (__c0 == V::Monster::FAKE_MERCHANT_MONSTER && __c1 == V::MoveId::ENRAGE_MOVE) =>
        {
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(ENRAGE));
            None
        }
        (monster, move_id) => V::generated_move(monster, move_id, step, fight),
    }
}
