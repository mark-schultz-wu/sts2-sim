//! The first act's monsters.

use sts2_core::{ContentError, Fight, MonsterSpawn, PowerAmount};

use super::super::super::generated::powers::Power;

use super::super::super::generated::ascension::AscensionLevel;
use super::super::super::generated::encounters::Encounter;
use super::super::super::generated::monster_moves;
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

/// Perform one of an Overgrowth monster's moves, where the generator left
/// it to a hand port.
pub(super) fn perform<F: Fight<V>>(monster: Monster, move_id: MoveId, fight: &mut F) {
    match (monster, move_id) {
        (Monster::ShrinkerBeetle, MoveId::ShrinkerMove) => {
            fight.apply_to_opponents(Power::ShrinkPower, PowerAmount::Endless)
        }
        (Monster::FuzzyWurmCrawler, MoveId::FirstAcidGoop | MoveId::AcidGoop) => {
            fight.attack_opponents()
        }
        (Monster::FuzzyWurmCrawler, MoveId::Inhale) => {
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(INHALE_STRENGTH))
        }
        (Monster::Fogmog, MoveId::IllusionMove) => illusion(fight),
        (monster, move_id) => monster_moves::perform(monster, move_id, fight),
    }
}

/// What the Fuzzy Wurm Crawler's Inhale adds to it.
///
/// A constant in the move itself. Its `BuffIntent` advertises none.
const INHALE_STRENGTH: Number = 7;

/// `Fogmog.IllusionMove`. Its illusion joins the front of the enemy roster.
fn illusion<F: Fight<V>>(fight: &mut F) {
    let eye = fight.summon(Monster::EyeWithTeeth, 0);
    fight.apply_power(eye, Power::IllusionPower, PowerAmount::Counted(1));
    fight.apply_power(eye, Power::MinionPower, PowerAmount::Counted(1));
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

/// `Vantom.SlipperyAmt`, either side of `ToughEnemies`.
fn slippery(ascension: AscensionLevel) -> Number {
    match ascension.has(AscensionLevel::ToughEnemies) {
        true => SLIPPERY_TOUGH,
        false => SLIPPERY,
    }
}

const SLIPPERY: Number = 8;
const SLIPPERY_TOUGH: Number = 9;
