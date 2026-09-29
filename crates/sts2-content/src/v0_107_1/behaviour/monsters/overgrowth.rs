//! The first act's monsters.

use sts2_core::{Amount, ContentError, Effect, Fight, MonsterSpawn, PowerAmount, Scaling};

use super::super::super::generated::powers::Power;

use super::super::super::generated::ascension::AscensionLevel;
use super::super::super::generated::encounters::Encounter;
use super::super::super::generated::monster_moves;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::{Number, V};

/// The powers an Overgrowth monster applies to itself as it spawns.
///
/// Six monsters of this act have an `AfterAddedToRoom` that applies one:
/// Byrdonis, the Phrog Parasite, the Bygone Effigy, the Vantom, each Inklet
/// and a Kin Follower.
///
/// Nothing for a monster this act does not port.
pub(super) fn starting_powers(
    monster: Monster,
    ascension: AscensionLevel,
) -> Option<Vec<(Power, PowerAmount<Number>)>> {
    Some(match monster {
        Monster::Byrdonis => vec![(Power::TerritorialPower, PowerAmount::Counted(1))],
        Monster::PhrogParasite => vec![(Power::InfestedPower, PowerAmount::Counted(4))],
        Monster::BygoneEffigy => vec![(Power::SlowPower, PowerAmount::Counted(1))],
        Monster::KinFollower => vec![(Power::MinionPower, PowerAmount::Counted(1))],
        Monster::Inklet => vec![(Power::SlipperyPower, PowerAmount::Counted(1))],
        Monster::Vantom => vec![(
            Power::SlipperyPower,
            PowerAmount::Counted(slippery(ascension)),
        )],
        // The Jaxfruit starts a sound loop.
        Monster::SnappingJaxfruit => Vec::new(),
        Monster::EyeWithTeeth => vec![(Power::IllusionPower, PowerAmount::Counted(1))],
        _ => return None,
    })
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
pub(super) fn perform<F: Fight<V>>(
    monster: Monster,
    move_id: MoveId,
    step: u8,
    fight: &mut F,
) -> Option<u8> {
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
        // The `TestMode.IsOff` block these open with only finds the creature
        // node.
        (Monster::SnappingJaxfruit, MoveId::EnergyOrbMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            fight.apply_to_actor(
                Power::StrengthPower,
                PowerAmount::Counted(ENERGY_ORB_STRENGTH),
            );
        }
        // `IsCrossbowReloaded`, which each of these two sets last, picks its
        // animation.
        (Monster::CrossbowRubyRaider, MoveId::FireMove) => fight.attack_opponents(),
        (Monster::CrossbowRubyRaider, MoveId::ReloadMove) => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(RELOAD_BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
        }
        // The Ceremonial Beast's moves set flags that pick its animation.
        (Monster::CeremonialBeast, MoveId::PlowMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(PLOW_STRENGTH));
        }
        (Monster::CeremonialBeast, MoveId::StompMove) => fight.attack_opponents(),
        (Monster::CeremonialBeast, MoveId::CrushMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            let strength = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => CRUSH_STRENGTH_DEADLY,
                false => CRUSH_STRENGTH,
            };
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(strength));
        }
        (Monster::CeremonialBeast, MoveId::StunMove) => {}
        // Three hits, which the builder the generator stops on only animates.
        (Monster::KinPriest, MoveId::BeamMove) => fight.attack_opponents(),
        // The speech it gives the first time is all `SpeechUsed` reads.
        (Monster::KinPriest, MoveId::RitualMove) => {
            let strength = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => RITUAL_STRENGTH_DEADLY,
                false => RITUAL_STRENGTH,
            };
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(strength));
        }
        (Monster::Flyconid, MoveId::VulnerableSporesMove) => {
            fight.apply_to_opponents(Power::VulnerablePower, PowerAmount::Counted(SPORES));
        }
        (Monster::Flyconid, MoveId::FrailSporesMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(Power::FrailPower, PowerAmount::Counted(SPORES));
        }
        (monster, move_id) => {
            return monster_moves::overgrowth::perform(monster, move_id, step, fight);
        }
    }
    None
}

/// The block the Crossbow Ruby Raider's Reload gives it, a constant in the
/// move.
const RELOAD_BLOCK: Number = 3;

/// `KinPriest.RitualStrength`, either side of `DeadlyEnemies`.
const RITUAL_STRENGTH: Number = 2;
const RITUAL_STRENGTH_DEADLY: Number = 3;

/// `CeremonialBeast.PlowStrength`.
const PLOW_STRENGTH: Number = 2;

/// `CeremonialBeast.CrushStrength`, either side of `DeadlyEnemies`.
const CRUSH_STRENGTH: Number = 3;
const CRUSH_STRENGTH_DEADLY: Number = 4;

/// What the Snapping Jaxfruit's Energy Orb adds to it, a constant in the move.
const ENERGY_ORB_STRENGTH: Number = 2;

/// The Vulnerable and the Frail a Flyconid's spores apply, constants in the
/// moves.
const SPORES: Number = 2;

/// What the Fuzzy Wurm Crawler's Inhale adds to it.
///
/// A constant in the move itself. Its `BuffIntent` advertises none.
const INHALE_STRENGTH: Number = 7;

/// `Fogmog.IllusionMove`: `CreatureCmd.Add<EyeWithTeeth>` into the
/// `illusion` slot, the first of `FogmogNormal.Slots`.
fn illusion<F: Fight<V>>(fight: &mut F) {
    fight.summon_into_slot(Monster::EyeWithTeeth, 0);
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
