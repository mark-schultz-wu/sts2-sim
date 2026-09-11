//! The first act's monsters.

use sts2_core::{ContentError, Fight, MonsterSpawn, PowerAmount};

use super::super::super::generated::powers::Power;

use super::super::super::generated::ascension::AscensionLevel;
use super::super::super::generated::cards::{Cards, StatusCard};
use super::super::super::generated::encounters::Encounter;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::V;

/// `INIT_MOVE`, the one conditional a Nibbit has.
///
/// `NibbitsWeak` builds one Nibbit and sets `IsAlone`, so it butts.
/// `NibbitsNormal` builds two and sets `IsFront` on the first, so the front
/// one slices and the back one hisses.
///
/// The alone branch is not in the table. `NIBBIT` was dumped not alone, and a
/// graph adds its branches from those same fields, so the dump holds the other
/// two and nothing else.
///
/// # Panics
///
/// A Nibbit an unported encounter built.
pub(super) fn nibbit_opens(at: u8, spawn: MonsterSpawn<Encounter>) -> Result<u8, ContentError> {
    let state = |wanted| super::state_of(Monster::Nibbit, wanted, at);
    match (
        spawn.encounter,
        spawn.roster_size.get(),
        spawn.position.get(),
    ) {
        (Encounter::NibbitsWeak, 1, 0) => state(MoveId::ButtMove),
        (Encounter::NibbitsNormal, 2, 0) => state(MoveId::SliceMove),
        (Encounter::NibbitsNormal, 2, 1) => state(MoveId::HissMove),
        _ => unimplemented!("a Nibbit spawned {spawn:?}"),
    }
}

/// How many copies of Slimed each spitting move leaves behind.
///
/// The move's own constant in the game. The intent beside it advertises the
/// same number, and the table carries that, but a status intent's count is not
/// what the move reads.
///
/// The two Sticky Shots are different moves that spit different amounts.
/// `LeafSlimeM` spits two and `TwigSlimeM` one.
const STICKY_CARDS: usize = 2;
const TWIG_STICKY_CARDS: usize = 1;
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

/// `TwigSlimeM`. Pounces for a clump, or spits one card, never twice running.
///
/// # Panics
///
/// A move of its graph nobody has ported.
pub(super) fn twig_slime_m<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::PokeyPounceMove => attack(fight),
        MoveId::StickyShotMove => fight.give_cards(SLIMED, TWIG_STICKY_CARDS),
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

/// `Nibbit`. Butts, or slices and blocks, or hisses itself stronger.
///
/// Slice's block and Hiss's Strength are constants in the moves themselves.
/// The table carries the numbers an intent advertises, and a `DefendIntent`
/// and a `BuffIntent` advertise none.
///
/// # Panics
///
/// A move of its graph nobody has ported.
pub(super) fn nibbit<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::ButtMove => attack(fight),
        MoveId::SliceMove => {
            attack(fight);
            let block = match fight.ascension().has(AscensionLevel::ToughEnemies) {
                true => SLICE_BLOCK_TOUGH,
                false => SLICE_BLOCK,
            };
            fight.gain_block(block);
        }
        MoveId::HissMove => {
            let strength = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => HISS_STRENGTH_DEADLY,
                false => HISS_STRENGTH,
            };
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(strength));
        }
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `Nibbit.SliceBlock`, either side of `ToughEnemies`.
const SLICE_BLOCK: i32 = 5;
const SLICE_BLOCK_TOUGH: i32 = 6;

/// `Nibbit.HissStrengthGain`, either side of `DeadlyEnemies`.
const HISS_STRENGTH: i32 = 2;
const HISS_STRENGTH_DEADLY: i32 = 3;

/// `DamageCmd.Attack(n).FromMonster(this)`, for the moves that are that and
/// nothing else.
fn attack<F: Fight<V>>(fight: &mut F) {
    let damage = fight.intent_damage();
    fight.attack_opponents(damage);
}
