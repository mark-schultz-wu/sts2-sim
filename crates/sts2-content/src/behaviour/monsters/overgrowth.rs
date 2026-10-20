//! The first act's monsters.

use sts2_core::{Amount, ContentError, Effect, Fight, MonsterSpawn, PowerAmount, Scaling};

use crate::behaviour::monsters::Number;
use crate::generated::ascension::AscensionLevel;
use crate::ids::{EncounterIds as _, MonsterIds as _, MoveIds as _, PowerIds as _};

/// The powers an Overgrowth monster applies to itself as it spawns.
///
/// Six monsters of this act have an `AfterAddedToRoom` that applies one:
/// Byrdonis, the Phrog Parasite, the Bygone Effigy, the Vantom, each Inklet
/// and a Kin Follower.
///
/// Nothing for a monster this act does not port.
pub(super) fn starting_powers<V: crate::Build>(
    monster: V::Monster,
    ascension: AscensionLevel,
) -> Option<Vec<(V::Power, PowerAmount<Number>)>> {
    Some(match monster {
        __c0 if __c0 == V::Monster::BYRDONIS => {
            vec![(V::Power::TERRITORIAL_POWER, PowerAmount::Counted(1))]
        }
        __c0 if __c0 == V::Monster::PHROG_PARASITE => {
            vec![(V::Power::INFESTED_POWER, PowerAmount::Counted(4))]
        }
        __c0 if __c0 == V::Monster::BYGONE_EFFIGY => {
            vec![(V::Power::SLOW_POWER, PowerAmount::Counted(1))]
        }
        __c0 if __c0 == V::Monster::KIN_FOLLOWER => {
            vec![(V::Power::MINION_POWER, PowerAmount::Counted(1))]
        }
        __c0 if __c0 == V::Monster::INKLET => {
            vec![(V::Power::SLIPPERY_POWER, PowerAmount::Counted(1))]
        }
        __c0 if __c0 == V::Monster::VANTOM => vec![(
            V::Power::SLIPPERY_POWER,
            PowerAmount::Counted(slippery(ascension)),
        )],
        // The Jaxfruit starts a sound loop.
        __c0 if __c0 == V::Monster::SNAPPING_JAXFRUIT => Vec::new(),
        __c0 if __c0 == V::Monster::EYE_WITH_TEETH => {
            vec![(V::Power::ILLUSION_POWER, PowerAmount::Counted(1))]
        }
        _ => return None,
    })
}

/// Which branch an Overgrowth monster's conditional state takes.
pub(super) fn conditional<V: crate::Build>(
    monster: V::Monster,
    at: u8,
    spawn: MonsterSpawn<V::Encounter>,
) -> Result<u8, ContentError> {
    match monster {
        __c0 if __c0 == V::Monster::NIBBIT => nibbit_opens::<V>(at, spawn),
        __c0 if __c0 == V::Monster::WRIGGLER => wriggler_opens::<V>(at, spawn),
        monster => unimplemented!("what {monster:?} asks the fight at state {at}"),
    }
}

/// A Wriggler alternates its opening by its place in the four-creature row.
fn wriggler_opens<V: crate::Build>(
    at: u8,
    spawn: MonsterSpawn<V::Encounter>,
) -> Result<u8, ContentError> {
    let wanted = match spawn.position.get() % 2 {
        0 => V::MoveId::NASTY_BITE_MOVE,
        _ => V::MoveId::WRIGGLE_MOVE,
    };
    crate::behaviour::monsters::state_of::<V>(V::Monster::WRIGGLER, wanted, at)
}

/// Perform one of an Overgrowth monster's moves, where the generator left
/// it to a hand port.
pub(super) fn perform<V: crate::Build, F: Fight<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    step: u8,
    fight: &mut F,
) -> Option<u8> {
    match (monster, move_id) {
        (__c0, __c1)
            if (__c0 == V::Monster::SHRINKER_BEETLE && __c1 == V::MoveId::SHRINKER_MOVE) =>
        {
            fight.apply_to_opponents(V::Power::SHRINK_POWER, PowerAmount::Endless)
        }
        (__c0, __c1)
            if (__c0 == V::Monster::FUZZY_WURM_CRAWLER
                && (__c1 == V::MoveId::FIRST_ACID_GOOP || __c1 == V::MoveId::ACID_GOOP)) =>
        {
            fight.attack_opponents()
        }
        (__c0, __c1) if (__c0 == V::Monster::FUZZY_WURM_CRAWLER && __c1 == V::MoveId::INHALE) => {
            fight.apply_to_actor(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(INHALE_STRENGTH),
            )
        }
        (__c0, __c1) if (__c0 == V::Monster::FOGMOG && __c1 == V::MoveId::ILLUSION_MOVE) => {
            illusion::<V, _>(fight)
        }
        // The `TestMode.IsOff` block these open with only finds the creature
        // node.
        (__c0, __c1)
            if (__c0 == V::Monster::SNAPPING_JAXFRUIT && __c1 == V::MoveId::ENERGY_ORB_MOVE) =>
        {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.apply_to_actor(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(ENERGY_ORB_STRENGTH),
            );
        }
        // `IsCrossbowReloaded`, which each of these two sets last, picks its
        // animation.
        (__c0, __c1)
            if (__c0 == V::Monster::CROSSBOW_RUBY_RAIDER && __c1 == V::MoveId::FIRE_MOVE) =>
        {
            fight.attack_opponents()
        }
        (__c0, __c1)
            if (__c0 == V::Monster::CROSSBOW_RUBY_RAIDER && __c1 == V::MoveId::RELOAD_MOVE) =>
        {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(RELOAD_BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
        }
        // The Ceremonial Beast's moves set flags that pick its animation.
        (__c0, __c1) if (__c0 == V::Monster::CEREMONIAL_BEAST && __c1 == V::MoveId::PLOW_MOVE) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.apply_to_actor(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(PLOW_STRENGTH),
            );
        }
        (__c0, __c1) if (__c0 == V::Monster::CEREMONIAL_BEAST && __c1 == V::MoveId::STOMP_MOVE) => {
            fight.attack_opponents()
        }
        (__c0, __c1) if (__c0 == V::Monster::CEREMONIAL_BEAST && __c1 == V::MoveId::CRUSH_MOVE) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            let strength = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => CRUSH_STRENGTH_DEADLY,
                false => CRUSH_STRENGTH,
            };
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(strength));
        }
        (__c0, __c1) if (__c0 == V::Monster::CEREMONIAL_BEAST && __c1 == V::MoveId::STUN_MOVE) => {}
        // Three hits, which the builder the generator stops on only animates.
        (__c0, __c1) if (__c0 == V::Monster::KIN_PRIEST && __c1 == V::MoveId::BEAM_MOVE) => {
            fight.attack_opponents()
        }
        // The speech it gives the first time is all `SpeechUsed` reads.
        (__c0, __c1) if (__c0 == V::Monster::KIN_PRIEST && __c1 == V::MoveId::RITUAL_MOVE) => {
            let strength = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => RITUAL_STRENGTH_DEADLY,
                false => RITUAL_STRENGTH,
            };
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(strength));
        }
        (__c0, __c1)
            if (__c0 == V::Monster::FLYCONID && __c1 == V::MoveId::VULNERABLE_SPORES_MOVE) =>
        {
            fight.apply_to_opponents(V::Power::VULNERABLE_POWER, PowerAmount::Counted(SPORES));
        }
        (__c0, __c1) if (__c0 == V::Monster::FLYCONID && __c1 == V::MoveId::FRAIL_SPORES_MOVE) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(V::Power::FRAIL_POWER, PowerAmount::Counted(SPORES));
        }
        (monster, move_id) => {
            return V::generated_move(monster, move_id, step, fight);
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
fn illusion<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.summon_into_slot(V::Monster::EYE_WITH_TEETH, 0);
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
fn nibbit_opens<V: crate::Build>(
    at: u8,
    spawn: MonsterSpawn<V::Encounter>,
) -> Result<u8, ContentError> {
    let state = |wanted| crate::behaviour::monsters::state_of::<V>(V::Monster::NIBBIT, wanted, at);
    match (
        spawn.encounter,
        spawn.roster_size.get(),
        spawn.position.get(),
    ) {
        (__c0, 1, 0) if __c0 == V::Encounter::NIBBITS_WEAK => state(V::MoveId::BUTT_MOVE),
        (__c0, 2, 0) if __c0 == V::Encounter::NIBBITS_NORMAL => state(V::MoveId::SLICE_MOVE),
        (__c0, 2, 1) if __c0 == V::Encounter::NIBBITS_NORMAL => state(V::MoveId::HISS_MOVE),
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
