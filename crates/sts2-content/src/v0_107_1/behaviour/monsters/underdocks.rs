//! The alternative first act's monsters.

use sts2_core::{ContentError, Fight, MonsterSpawn, PowerAmount};

use super::super::super::generated::ascension::AscensionLevel;
use super::super::super::generated::encounters::Encounter;
use super::super::super::generated::monster_moves;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::super::super::generated::powers::Power;
use super::{Number, V};

/// `Toadpole.SpikenAmount`, the Thorns Spike Spit takes back off itself.
const SPIKEN: Number = 2;

/// `CorpseSlug.RavenousStr`, either side of `DeadlyEnemies`.
const RAVENOUS: Number = 4;
const RAVENOUS_DEADLY: Number = 5;

/// `TwoTailedRat._turnsUntilSummonable`, which each of its other moves takes
/// one off.
pub(super) const RAT_SUMMONABLE_IN: u8 = 2;

/// `_callForBackupLimit`: the summons a rat makes in a fight.
const RAT_BACKUPS: u8 = 3;

/// `ScreechMove`'s `PowerCmd.Apply<FrailPower>(…, 1m, …)`.
const SCREECH_FRAIL: Number = 1;

/// `SewerClam.AfterAddedToRoom`'s Plating, either side of `ToughEnemies`.
const CLAM_PLATING: Number = 8;
const CLAM_PLATING_TOUGH: Number = 9;

/// `FossilStalker.AfterAddedToRoom`'s Suck.
const STALKER_SUCK: Number = 3;

/// `SkulkingColony.AfterAddedToRoom`'s Hardened Shell.
const COLONY_SHELL: Number = 20;

/// `TerrorEel.ShriekAmount`, either side of `ToughEnemies`.
const EEL_SHRIEK: Number = 70;
const EEL_SHRIEK_TOUGH: Number = 75;

/// The powers an Underdocks monster applies to itself as it spawns.
///
/// Nothing for a monster this act does not port.
pub(super) fn starting_powers(
    monster: Monster,
    ascension: AscensionLevel,
) -> Option<Vec<(Power, PowerAmount<Number>)>> {
    Some(match monster {
        Monster::CorpseSlug => vec![(
            Power::RavenousPower,
            PowerAmount::Counted(match ascension.has(AscensionLevel::DeadlyEnemies) {
                true => RAVENOUS_DEADLY,
                false => RAVENOUS,
            }),
        )],
        // Each override calls the base and nothing else.
        Monster::Seapunk | Monster::TwoTailedRat => Vec::new(),
        Monster::SewerClam => vec![(
            Power::PlatingPower,
            PowerAmount::Counted(match ascension.has(AscensionLevel::ToughEnemies) {
                true => CLAM_PLATING_TOUGH,
                false => CLAM_PLATING,
            }),
        )],
        Monster::FossilStalker => vec![(Power::SuckPower, PowerAmount::Counted(STALKER_SUCK))],
        Monster::GasBomb => vec![(Power::MinionPower, PowerAmount::Counted(1))],
        Monster::SkulkingColony => vec![(
            Power::HardenedShellPower,
            PowerAmount::Counted(COLONY_SHELL),
        )],
        Monster::TerrorEel => vec![(
            Power::ShriekPower,
            PowerAmount::Counted(match ascension.has(AscensionLevel::ToughEnemies) {
                true => EEL_SHRIEK_TOUGH,
                false => EEL_SHRIEK,
            }),
        )],
        _ => return None,
    })
}

/// Which branch an Underdocks monster's conditional state takes.
pub(super) fn conditional(
    monster: Monster,
    at: u8,
    spawn: MonsterSpawn<Encounter>,
) -> Result<u8, ContentError> {
    match monster {
        Monster::Toadpole => toadpole_opens(at, spawn),
        monster => unimplemented!("what {monster:?} asks the fight at state {at}"),
    }
}

/// `INIT_MOVE`, which reads `IsFront`. `ToadpolesWeak` builds two Toadpoles
/// and sets it on the first, so the front one opens on Spiken and the back
/// one on Whirl.
///
/// # Panics
///
/// A Toadpole an unported encounter built.
fn toadpole_opens(at: u8, spawn: MonsterSpawn<Encounter>) -> Result<u8, ContentError> {
    let state = |wanted| super::state_of(Monster::Toadpole, wanted, at);
    match (
        spawn.encounter,
        spawn.roster_size.get(),
        spawn.position.get(),
    ) {
        (Encounter::ToadpolesWeak, 2, 0) => state(MoveId::SpikenMove),
        (Encounter::ToadpolesWeak, 2, 1) => state(MoveId::WhirlMove),
        _ => unimplemented!("a Toadpole spawned {spawn:?}"),
    }
}

/// Perform one of an Underdocks monster's moves, where the generator left it
/// to a hand port.
pub(super) fn perform<F: Fight<V>>(
    monster: Monster,
    move_id: MoveId,
    step: u8,
    fight: &mut F,
) -> Option<u8> {
    match (monster, move_id) {
        // `DarkStrikeMove`: its hit. `BeforeDamage` plays its sound alone.
        (Monster::CalcifiedCultist | Monster::DampCultist, MoveId::DarkStrikeMove) => {
            fight.attack_opponents()
        }
        // `ScratchMove` and `DiseaseBiteMove`: a turn off the wait for a
        // summon, then the hit.
        (Monster::TwoTailedRat, MoveId::ScratchMove | MoveId::DiseaseBiteMove) => {
            rat_waits(fight);
            fight.attack_opponents();
        }
        (Monster::TwoTailedRat, MoveId::ScreechMove) => {
            rat_waits(fight);
            fight.apply_to_opponents(Power::FrailPower, PowerAmount::Counted(SCREECH_FRAIL));
        }
        (Monster::TwoTailedRat, MoveId::CallForBackupMove) => call_for_backup(fight),
        // `BloatMove`: a Gas Bomb into the first slot no enemy holds, for each
        // of its `BloatAmount`, which stays at one. Then its hit.
        (Monster::LivingFog, MoveId::BloatMove) => {
            fight.summon_into_first_free_slot(Monster::GasBomb);
            fight.attack_opponents();
        }
        // `ExplodeMove`: its hit, then `CreatureCmd.Kill` on itself.
        // `HasExploded` picks how it dies on screen alone.
        (Monster::GasBomb, MoveId::ExplodeMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            let bomb = fight.actor();
            fight.kill(bomb);
        }
        // `SpikeSpitMove`: the Thorns Spiken gave, taken back, then its hits.
        (Monster::Toadpole, MoveId::SpikeSpitMove) => {
            fight.apply_to_actor(Power::ThornsPower, PowerAmount::Counted(-SPIKEN));
            fight.attack_opponents();
        }
        (monster, move_id) => {
            return monster_moves::underdocks::perform(monster, move_id, step, fight);
        }
    }
    None
}

/// `TurnsUntilSummonable--`.
fn rat_waits<F: Fight<V>>(fight: &mut F) {
    fight.model(|model| {
        let super::Model::TwoTailedRat { summonable_in, .. } = model else {
            unreachable!("a Two-Tailed Rat's model is its own")
        };
        *summonable_in = summonable_in.saturating_sub(1);
    });
}

/// `CallForBackup`: a rat into the last slot no enemy holds. Then every rat,
/// the new one included, counts one past the highest count any of them held.
fn call_for_backup<F: Fight<V>>(fight: &mut F) {
    fight.summon_into_last_free_slot(Monster::TwoTailedRat);
    let mut highest = 0;
    fight.enemy_models(|model| {
        if let super::Model::TwoTailedRat { backups, .. } = *model {
            highest = highest.max(backups + 1);
        }
    });
    fight.enemy_models(|model| {
        if let super::Model::TwoTailedRat { backups, .. } = model {
            *backups = highest;
        }
    });
}

/// A Two-Tailed Rat's `RAND` branches, in the order it adds them. Scratch,
/// Disease Bite and Screech weigh 1 while it cannot summon and 1/12 while it
/// can. Call for Backup weighs 0.75 while it can, and nothing otherwise.
pub(super) fn rat_weight(
    at: usize,
    spawn: MonsterSpawn<Encounter>,
    model: &super::Model,
    advertised: impl Fn(MoveId) -> bool,
) -> f32 {
    let super::Model::TwoTailedRat {
        summonable_in,
        backups,
    } = *model
    else {
        unreachable!("a Two-Tailed Rat's model is its own")
    };
    // `CanSummon`, in its own order.
    let can_summon = summonable_in == 0
        && backups < RAT_BACKUPS
        && spawn.next_slot
        && !advertised(MoveId::CallForBackupMove);
    match (at, can_summon) {
        (0..=2, false) => 1.0,
        (0..=2, true) => 1.0 / 12.0,
        (3, true) => 0.75,
        (3, false) => 0.0,
        (at, _) => unreachable!("a Two-Tailed Rat has four branches, not {at}"),
    }
}

/// A move's damage that an Underdocks monster works out as it rolls.
///
/// `GasBomb.ExplodeDamage`, behind its `DeathBlowIntent`'s lambda: eight, or
/// nine at `DeadlyEnemies`.
pub(super) fn computed_damage(monster: Monster, move_id: MoveId, ascension: u8) -> i32 {
    let deadly = ascension >= AscensionLevel::DeadlyEnemies as u8;
    match (monster, move_id) {
        (Monster::GasBomb, MoveId::ExplodeMove) => match deadly {
            true => 9,
            false => 8,
        },
        (monster, move_id) => unimplemented!("the damage {monster:?} works out for {move_id:?}"),
    }
}
