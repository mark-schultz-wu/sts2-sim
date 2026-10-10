//! The alternative first act's monsters.

use sts2_core::{Amount, ContentError, Effect, Fight, MonsterSpawn, PowerAmount, Scaling};

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

/// `WaterfallGiant.PressureGunIncrease`.
const GIANT_GUN_RAISE: i32 = 5;

/// The Steam Eruption each Waterfall Giant attack and its Siphon build.
const GIANT_STEAM: Number = 3;

/// `SoulFysh.FadeMove`'s Intangible, and `ScreamMoveAmount`'s Vulnerable.
const FYSH_FADE: Number = 2;
const FYSH_SCREAM: Number = 3;

/// The Beckon the Soul Fysh deals the player.
fn beckon() -> super::super::super::generated::cards::Cards {
    use super::super::super::generated::cards::{Cards, StatusCard};

    Cards::Status(StatusCard::Beckon)
}

/// `LagavulinMatriarch.Sleep`'s Plating and Asleep.
const MATRIARCH_PLATING: Number = 12;
const MATRIARCH_ASLEEP: Number = 3;

/// `SoulSiphonMove`'s Strength and Dexterity off each target, and the
/// Strength it takes for itself.
const MATRIARCH_SIPHON: Number = 2;

/// `PhantasmalGardener.SkittishAmount`, either side of `ToughEnemies`.
const GARDENER_SKITTISH: Number = 6;
const GARDENER_SKITTISH_TOUGH: Number = 7;

/// `PhantasmalGardener.EnlargeStr`, either side of `DeadlyEnemies`.
const GARDENER_ENLARGE: Number = 2;
const GARDENER_ENLARGE_DEADLY: Number = 3;

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
        // Each override calls the base and nothing else. The Waterfall Giant's
        // also sets its Pressure Gun's damage, which its model counts from.
        Monster::Seapunk | Monster::TwoTailedRat | Monster::WaterfallGiant => Vec::new(),
        Monster::SewerClam => vec![(
            Power::PlatingPower,
            PowerAmount::Counted(match ascension.has(AscensionLevel::ToughEnemies) {
                true => CLAM_PLATING_TOUGH,
                false => CLAM_PLATING,
            }),
        )],
        Monster::FossilStalker => vec![(Power::SuckPower, PowerAmount::Counted(STALKER_SUCK))],
        Monster::GasBomb => vec![(Power::MinionPower, PowerAmount::Counted(1))],
        // `Sleep`: Plating, then Asleep.
        Monster::LagavulinMatriarch => vec![
            (Power::PlatingPower, PowerAmount::Counted(MATRIARCH_PLATING)),
            (Power::AsleepPower, PowerAmount::Counted(MATRIARCH_ASLEEP)),
        ],
        Monster::PhantasmalGardener => vec![(
            Power::SkittishPower,
            PowerAmount::Counted(match ascension.has(AscensionLevel::ToughEnemies) {
                true => GARDENER_SKITTISH_TOUGH,
                false => GARDENER_SKITTISH,
            }),
        )],
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
        // `PressurizeMove`: Steam Eruption on itself.
        (Monster::WaterfallGiant, MoveId::PressurizeMove) => {
            let steam = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => 20,
                false => 15,
            };
            fight.apply_to_actor(Power::SteamEruptionPower, PowerAmount::Counted(steam));
        }
        // `RamMove` and `PressureUpMove`: the hit, then three Steam Eruption.
        (Monster::WaterfallGiant, MoveId::RamMove | MoveId::PressureUpMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            fight.apply_to_actor(Power::SteamEruptionPower, PowerAmount::Counted(GIANT_STEAM));
        }
        // `StompMove`: the hit, a Weak on each target, then three Steam
        // Eruption.
        (Monster::WaterfallGiant, MoveId::StompMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(Power::WeakPower, PowerAmount::Counted(1));
            fight.apply_to_actor(Power::SteamEruptionPower, PowerAmount::Counted(GIANT_STEAM));
        }
        // `SiphonMove`: `SiphonHeal` for each player, of which a fight here
        // has one, then three Steam Eruption.
        (Monster::WaterfallGiant, MoveId::SiphonMove) => {
            let heal = match fight.ascension().has(AscensionLevel::ToughEnemies) {
                true => 15,
                false => 10,
            };
            let giant = fight.actor();
            fight.heal(giant, heal);
            fight.apply_to_actor(Power::SteamEruptionPower, PowerAmount::Counted(GIANT_STEAM));
        }
        // `PressureGunMove`: the hit, five more on the next, then three Steam
        // Eruption.
        (Monster::WaterfallGiant, MoveId::PressureGunMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            fight.model(|model| {
                let super::Model::WaterfallGiant { gun_raises, .. } = model else {
                    unreachable!("a Waterfall Giant's model is its own")
                };
                *gun_raises += 1;
            });
            fight.apply_to_actor(Power::SteamEruptionPower, PowerAmount::Counted(GIANT_STEAM));
        }
        // `AboutToBlowMove`: the Steam Eruption it built becomes the damage
        // its Explode deals, and comes off.
        (Monster::WaterfallGiant, MoveId::AboutToBlowMove) => {
            use sts2_core::CreatureView as _;

            let giant = fight.actor();
            let built = fight
                .creatures()
                .find(|creature| creature.id() == giant)
                .and_then(|creature| creature.power(Power::SteamEruptionPower))
                .map_or(0, |amount| match amount {
                    PowerAmount::Counted(amount) => amount,
                    PowerAmount::Endless => unimplemented!("an endless Steam Eruption"),
                });
            fight.model(|model| {
                let super::Model::WaterfallGiant { eruption, .. } = model else {
                    unreachable!("a Waterfall Giant's model is its own")
                };
                *eruption = built;
            });
            fight.remove_power(giant, Power::SteamEruptionPower);
        }
        // `ExplodeMove`: the hit, then `CreatureCmd.Kill` on itself.
        (Monster::WaterfallGiant, MoveId::ExplodeMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            let giant = fight.actor();
            fight.kill(giant);
        }
        // `BeckonMove`: for the one player, a Beckon into the draw pile at
        // random, then one into the discard pile.
        (Monster::SoulFysh, MoveId::BeckonMove) => {
            fight.add_at_random(beckon(), sts2_core::Pile::Draw);
            fight.add_generated_cards(
                sts2_core::CardIdent::new(beckon()),
                sts2_core::Pile::Discard,
                1,
            );
        }
        // `GazeMove`: its hit, then a Beckon into the discard pile.
        (Monster::SoulFysh, MoveId::GazeMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            fight.add_generated_cards(
                sts2_core::CardIdent::new(beckon()),
                sts2_core::Pile::Discard,
                1,
            );
        }
        // `FadeMove`: two Intangible. `IsInvisible` and the music are on
        // screen alone.
        (Monster::SoulFysh, MoveId::FadeMove) => {
            fight.apply_to_actor(Power::IntangiblePower, PowerAmount::Counted(FYSH_FADE));
        }
        // `ScreamMove`: its hit, then three Vulnerable on each target.
        (Monster::SoulFysh, MoveId::ScreamMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(Power::VulnerablePower, PowerAmount::Counted(FYSH_SCREAM));
        }
        // The Matriarch's four waking moves. Each first stops its sleeping
        // effect, which is all on screen.
        (Monster::LagavulinMatriarch, MoveId::SlashMove | MoveId::DisembowelMove) => {
            fight.attack_opponents()
        }
        // `Slash2Move`: its hit, then powered block on itself.
        (Monster::LagavulinMatriarch, MoveId::Slash2Move) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            let amount = match fight.ascension().has(AscensionLevel::ToughEnemies) {
                true => 14,
                false => 12,
            };
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(amount),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
        }
        // `SoulSiphonMove`: Strength and Dexterity off each target, then the
        // Strength on itself.
        (Monster::LagavulinMatriarch, MoveId::SoulSiphonMove) => {
            fight.apply_to_opponents(
                Power::StrengthPower,
                PowerAmount::Counted(-MATRIARCH_SIPHON),
            );
            fight.apply_to_opponents(
                Power::DexterityPower,
                PowerAmount::Counted(-MATRIARCH_SIPHON),
            );
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(MATRIARCH_SIPHON));
        }
        // `EnlargeMove`: Strength on itself. `EnlargeTriggers` and the scale
        // it sets size it on screen alone.
        (Monster::PhantasmalGardener, MoveId::EnlargeMove) => {
            let strength = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => GARDENER_ENLARGE_DEADLY,
                false => GARDENER_ENLARGE,
            };
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(strength));
        }
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
/// nine at `DeadlyEnemies`. The Waterfall Giant reads its own model.
pub(super) fn computed_damage(
    monster: Monster,
    move_id: MoveId,
    ascension: u8,
    model: &super::Model,
) -> i32 {
    let deadly = ascension >= AscensionLevel::DeadlyEnemies as u8;
    match (monster, move_id) {
        // `CurrentPressureGunDamage`: its base, and five for each Pressure
        // Gun since.
        (Monster::WaterfallGiant, MoveId::PressureGunMove) => {
            let super::Model::WaterfallGiant { gun_raises, .. } = *model else {
                unreachable!("a Waterfall Giant's model is its own")
            };
            (if deadly { 23 } else { 20 }) + GIANT_GUN_RAISE * i32::from(gun_raises)
        }
        // `SteamEruptionDamage`, which About to Blow saved.
        (Monster::WaterfallGiant, MoveId::ExplodeMove) => {
            let super::Model::WaterfallGiant { eruption, .. } = *model else {
                unreachable!("a Waterfall Giant's model is its own")
            };
            eruption
        }
        (Monster::GasBomb, MoveId::ExplodeMove) => match deadly {
            true => 9,
            false => 8,
        },
        (monster, move_id) => unimplemented!("the damage {monster:?} works out for {move_id:?}"),
    }
}

/// The Matriarch's `SLEEP_BRANCH`: asleep while it holds Asleep, and slashing
/// once it does not.
pub(super) fn matriarch_branch<P: sts2_core::Printed<V> + ?Sized>(
    at: u8,
    powers: &[sts2_core::Listener<'_, V, P>],
) -> Result<u8, ContentError> {
    let asleep = powers.iter().any(|listener| {
        matches!(
            listener,
            sts2_core::Listener::Power {
                power: Power::AsleepPower,
                ..
            }
        )
    });
    let wanted = match asleep {
        true => MoveId::SleepMove,
        false => MoveId::SlashMove,
    };
    super::state_of(Monster::LagavulinMatriarch, wanted, at)
}
