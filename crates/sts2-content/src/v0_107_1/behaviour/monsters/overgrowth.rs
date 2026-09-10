//! The first act's monsters.

use sts2_core::{Fight, PowerAmount};

use super::super::super::generated::powers::Power;

use super::super::super::generated::cards::{Cards, StatusCard};
use super::super::super::generated::monsters::MoveId;
use super::V;

/// How many copies of Slimed each spitting move leaves behind.
///
/// The move's own constant in the game. The intent beside it advertises the
/// same number, and the table carries that, but a status intent's count is not
/// what the move reads.
const STICKY_CARDS: usize = 2;
const GOOP_CARDS: usize = 1;

/// The card every slime spits.
const SLIMED: Cards = Cards::Status(StatusCard::Slimed);

/// `LeafSlimeM`. Alternates a spit for two cards and a clump of damage.
///
/// # Panics
///
/// A move of its graph nobody has ported.
pub(super) fn leaf_slime_m<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::ClumpShot => attack(fight),
        MoveId::StickyShot => fight.give_cards(SLIMED, STICKY_CARDS),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `LeafSlimeS`. A spit for one card or a tackle, never twice running.
///
/// # Panics
///
/// A move of its graph nobody has ported.
pub(super) fn leaf_slime_s<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::TackleMove => attack(fight),
        MoveId::GoopMove => fight.give_cards(SLIMED, GOOP_CARDS),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `TwigSlimeS`. Tackles, and nothing else.
///
/// # Panics
///
/// A move of its graph nobody has ported.
pub(super) fn twig_slime_s<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::TackleMove => attack(fight),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `ShrinkerBeetle`. Shrinks once, then chews and stamps by turns.
///
/// # Panics
///
/// A move of its graph nobody has ported.
pub(super) fn shrinker_beetle<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::ShrinkerMove => fight.apply_to_opponents(Power::ShrinkPower, PowerAmount::Endless),
        MoveId::ChompMove | MoveId::StompMove => attack(fight),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `DamageCmd.Attack(n).FromMonster(this)`, for the moves that are that and
/// nothing else.
fn attack<F: Fight<V>>(fight: &mut F) {
    let damage = fight.intent_damage();
    fight.attack_opponents(damage);
}
