//! The first act's monsters.

use sts2_core::{Amount, ContentError, Effect, Fight, MonsterSpawn, PowerAmount, Scaling};

use super::super::super::generated::powers::Power;

use super::super::super::generated::ascension::AscensionLevel;
use super::super::super::generated::cards::{Cards, StatusCard};
use super::super::super::generated::encounters::Encounter;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::{Number, V};

/// The powers an Overgrowth monster applies to itself as it spawns.
///
/// Four monsters of this act have an `AfterAddedToRoom`. Byrdonis opens
/// territorial, the Phrog Parasite opens infested, the Bygone Effigy opens
/// slow, and the Vantom opens slippery.
pub(super) fn starting_powers(
    monster: Monster,
    ascension: AscensionLevel,
) -> Vec<(Power, PowerAmount<Number>)> {
    match monster {
        Monster::Byrdonis => vec![(Power::TerritorialPower, PowerAmount::Counted(1))],
        Monster::PhrogParasite => vec![(Power::InfestedPower, PowerAmount::Counted(4))],
        Monster::BygoneEffigy => vec![(Power::SlowPower, PowerAmount::Counted(1))],
        Monster::Vantom => vec![(
            Power::SlipperyPower,
            PowerAmount::Counted(slippery(ascension)),
        )],
        _ => Vec::new(),
    }
}

/// Which branch an Overgrowth monster's conditional state takes.
pub(super) fn conditional(
    monster: Monster,
    at: u8,
    spawn: MonsterSpawn<Encounter>,
) -> Result<u8, ContentError> {
    match monster {
        Monster::Nibbit => nibbit_opens(at, spawn),
        Monster::Wriggler => wriggler_opens(at, spawn),
        monster => unimplemented!("what {monster:?} asks the fight at state {at}"),
    }
}

/// A Wriggler alternates its opening by its place in the four-creature row.
fn wriggler_opens(at: u8, spawn: MonsterSpawn<Encounter>) -> Result<u8, ContentError> {
    let wanted = match spawn.position.get() % 2 {
        0 => MoveId::NastyBiteMove,
        _ => MoveId::WriggleMove,
    };
    super::state_of(Monster::Wriggler, wanted, at)
}

/// Perform one of an Overgrowth monster's moves.
pub(super) fn perform<F: Fight<V>>(monster: Monster, move_id: MoveId, fight: &mut F) {
    match monster {
        Monster::LeafSlimeM => leaf_slime_m(move_id, fight),
        Monster::ShrinkerBeetle => shrinker_beetle(move_id, fight),
        Monster::FuzzyWurmCrawler => fuzzy_wurm_crawler(move_id, fight),
        Monster::LeafSlimeS => leaf_slime_s(move_id, fight),
        Monster::TwigSlimeM => twig_slime_m(move_id, fight),
        Monster::TwigSlimeS => twig_slime_s(move_id, fight),
        Monster::Nibbit => nibbit(move_id, fight),
        Monster::Fogmog => fogmog(move_id, fight),
        Monster::EyeWithTeeth => eye_with_teeth(move_id, fight),
        Monster::Byrdonis => byrdonis(move_id, fight),
        Monster::Mawler => mawler(move_id, fight),
        Monster::PhrogParasite => phrog_parasite(move_id, fight),
        Monster::BygoneEffigy => bygone_effigy(move_id, fight),
        Monster::Vantom => vantom(move_id, fight),
        Monster::Wriggler => wriggler(move_id, fight),
        monster => unimplemented!("what {monster:?} does when it moves"),
    }
}

/// A newly hatched Wriggler spends its first move arriving. It then bites, or
/// grows stronger and leaves an Infection behind.
fn wriggler<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::SpawnedMove => {}
        MoveId::NastyBiteMove => fight.attack_opponents(),
        MoveId::WriggleMove => {
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(WRIGGLE_STRENGTH));
            fight.give_cards(Cards::Status(StatusCard::Infection), WRIGGLE_CARDS);
        }
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

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
fn nibbit_opens(at: u8, spawn: MonsterSpawn<Encounter>) -> Result<u8, ContentError> {
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

/// How many Infections the Phrog Parasite leaves behind.
const INFECTION_CARDS: usize = 3;

/// What Wriggle adds to its owner and the other side.
const WRIGGLE_STRENGTH: Number = 2;
const WRIGGLE_CARDS: usize = 1;

/// The card every slime spits.
const SLIMED: Cards = Cards::Status(StatusCard::Slimed);

/// `PhrogParasite`. Infects the other side, or lashes it.
///
/// # Panics
///
/// A move of its graph nobody has ported.
fn phrog_parasite<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::InfectMove => {
            fight.give_cards(Cards::Status(StatusCard::Infection), INFECTION_CARDS)
        }
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// What waking up adds to the Bygone Effigy.
const WAKE_STRENGTH: Number = 10;

/// `BygoneEffigy`. Sleeps, wakes far stronger, and slashes from then on.
///
/// Its two sleeps do nothing at all. The first is the move it enters the
/// fight on, and the wake that follows is the only thing it does once.
///
/// # Panics
///
/// A move of its graph nobody has ported.
fn bygone_effigy<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::SleepMove | MoveId::SleepMove2 => {}
        MoveId::WakeMove => {
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(WAKE_STRENGTH));
        }
        MoveId::SlashesMove => fight.attack_opponents(),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `Vantom.SlipperyAmt`, either side of `ToughEnemies`.
fn slippery(ascension: AscensionLevel) -> Number {
    match ascension.has(AscensionLevel::ToughEnemies) {
        true => SLIPPERY_TOUGH,
        false => SLIPPERY,
    }
}

const SLIPPERY: Number = 8;
const SLIPPERY_TOUGH: Number = 9;

/// What one Prepare adds to the Vantom.
const PREPARE_STRENGTH: Number = 2;

/// How many Wounds a Dismember leaves behind.
const DISMEMBER_WOUNDS: usize = 3;

/// `Vantom`. Blots, lances twice, dismembers, prepares, and round again.
///
/// # Panics
///
/// A move of its graph nobody has ported.
fn vantom<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::InkBlotMove | MoveId::InkyLanceMove => fight.attack_opponents(),
        MoveId::DismemberMove => {
            fight.attack_opponents();
            fight.give_cards(Cards::Status(StatusCard::Wound), DISMEMBER_WOUNDS);
        }
        MoveId::PrepareMove => {
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(PREPARE_STRENGTH));
        }
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `LeafSlimeM`. Alternates a spit for two cards and a clump of damage.
///
/// # Panics
///
/// A move of its graph nobody has ported.
fn leaf_slime_m<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::ClumpShot => fight.attack_opponents(),
        MoveId::StickyShot => fight.give_cards(SLIMED, STICKY_CARDS),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `LeafSlimeS`. A spit for one card or a tackle, never twice running.
///
/// # Panics
///
/// A move of its graph nobody has ported.
fn leaf_slime_s<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::TackleMove => fight.attack_opponents(),
        MoveId::GoopMove => fight.give_cards(SLIMED, GOOP_CARDS),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `TwigSlimeM`. Pounces for a clump, or spits one card, never twice running.
///
/// # Panics
///
/// A move of its graph nobody has ported.
fn twig_slime_m<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::PokeyPounceMove => fight.attack_opponents(),
        MoveId::StickyShotMove => fight.give_cards(SLIMED, TWIG_STICKY_CARDS),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `TwigSlimeS`. Tackles, and nothing else.
///
/// # Panics
///
/// A move of its graph nobody has ported.
fn twig_slime_s<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::TackleMove => fight.attack_opponents(),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `ShrinkerBeetle`. Shrinks once, then chews and stamps by turns.
///
/// # Panics
///
/// A move of its graph nobody has ported.
fn shrinker_beetle<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::ShrinkerMove => fight.apply_to_opponents(Power::ShrinkPower, PowerAmount::Endless),
        MoveId::ChompMove | MoveId::StompMove => fight.attack_opponents(),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `FuzzyWurmCrawler`. Spits acid, and draws a breath between spits that
/// makes it stronger.
///
/// Inhale's Strength is a constant in the move itself. Its `BuffIntent`
/// advertises none.
///
/// # Panics
///
/// A move of its graph nobody has ported.
fn fuzzy_wurm_crawler<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    const INHALE_STRENGTH: Number = 7;

    match move_id {
        MoveId::FirstAcidGoop | MoveId::AcidGoop => fight.attack_opponents(),
        MoveId::Inhale => {
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(INHALE_STRENGTH))
        }
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
fn nibbit<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::ButtMove => fight.attack_opponents(),
        MoveId::SliceMove => {
            fight.attack_opponents();
            let block = match fight.ascension().has(AscensionLevel::ToughEnemies) {
                true => SLICE_BLOCK_TOUGH,
                false => SLICE_BLOCK,
            };
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
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
const SLICE_BLOCK: Number = 5;
const SLICE_BLOCK_TOUGH: Number = 6;

/// `Nibbit.HissStrengthGain`, either side of `DeadlyEnemies`.
const HISS_STRENGTH: Number = 2;
const HISS_STRENGTH_DEADLY: Number = 3;

/// `Fogmog`. Its illusion joins the front of the enemy roster. Both swipes
/// hit and make the Fogmog stronger afterwards.
fn fogmog<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::IllusionMove => {
            let eye = fight.summon(Monster::EyeWithTeeth, 0);
            fight.apply_power(eye, Power::IllusionPower, PowerAmount::Counted(1));
            fight.apply_power(eye, Power::MinionPower, PowerAmount::Counted(1));
        }
        MoveId::SwipeMove | MoveId::SwipeRandomMove => {
            fight.attack_opponents();
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(1));
        }
        MoveId::HeadbuttMove => fight.attack_opponents(),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `EyeWithTeeth`. Distracts by putting three Dazed into the discard pile.
fn eye_with_teeth<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::DistractMove => fight.give_cards(Cards::Status(StatusCard::Dazed), 3),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `Byrdonis`. Pecks three times, then swoops, and back again.
///
/// Both moves are an attack and nothing else. The peck's hit count is the
/// `PeckRepeat` its intent advertises.
///
/// # Panics
///
/// A move of its graph nobody has ported.
fn byrdonis<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::PeckMove | MoveId::SwoopMove => fight.attack_opponents(),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// `Mawler`. Both clawing moves attack. Its one roar leaves the player
/// Vulnerable for three turns.
///
/// # Panics
///
/// A move of its graph nobody has ported.
fn mawler<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::ClawMove | MoveId::RipAndTearMove => fight.attack_opponents(),
        MoveId::RoarMove => {
            fight.apply_to_opponents(Power::VulnerablePower, PowerAmount::Counted(3));
        }
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}
