//! The alternative first act's monsters.

use sts2_core::{Amount, ContentError, Effect, Fight, MonsterSpawn, PowerAmount, Scaling};

use crate::behaviour::monsters::Number;
use crate::generated::ascension::AscensionLevel;
use crate::ids::{CardIds as _, EncounterIds as _, MonsterIds as _, MoveIds as _, PowerIds as _};

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
/// `ThieveryPower`'s stack as the Gremlin Merc applies it.
const THIEVERY: Number = 20;
/// `DoubleSmashMove`'s Weak, and `HeheMove`'s Strength.
const MERC_WEAK: Number = 2;
const MERC_STRENGTH: Number = 2;

const FYSH_FADE: Number = 2;
const FYSH_SCREAM: Number = 3;

/// The Beckon the Soul Fysh deals the player.
fn beckon<V: crate::Build>() -> V::Card {
    V::Card::BECKON
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
pub(super) fn starting_powers<V: crate::Build>(
    monster: V::Monster,
    ascension: AscensionLevel,
) -> Option<Vec<(V::Power, PowerAmount<Number>)>> {
    Some(match monster {
        __c0 if __c0 == V::Monster::CORPSE_SLUG => vec![(
            V::Power::RAVENOUS_POWER,
            PowerAmount::Counted(match ascension.has(AscensionLevel::DeadlyEnemies) {
                true => RAVENOUS_DEADLY,
                false => RAVENOUS,
            }),
        )],
        // Each override calls the base and nothing else. The Waterfall Giant's
        // also sets its Pressure Gun's damage, which its model counts from.
        __c0 if __c0 == V::Monster::SEAPUNK
            || __c0 == V::Monster::TWO_TAILED_RAT
            || __c0 == V::Monster::WATERFALL_GIANT =>
        {
            Vec::new()
        }
        // `SurprisePower`, then a Thievery aimed at each player, of which a
        // fight here has one.
        __c0 if __c0 == V::Monster::GREMLIN_MERC => vec![
            (V::Power::SURPRISE_POWER, PowerAmount::Counted(1)),
            (V::Power::THIEVERY_POWER, PowerAmount::Counted(THIEVERY)),
        ],
        __c0 if __c0 == V::Monster::SEWER_CLAM => vec![(
            V::Power::PLATING_POWER,
            PowerAmount::Counted(match ascension.has(AscensionLevel::ToughEnemies) {
                true => CLAM_PLATING_TOUGH,
                false => CLAM_PLATING,
            }),
        )],
        __c0 if __c0 == V::Monster::FOSSIL_STALKER => {
            vec![(V::Power::SUCK_POWER, PowerAmount::Counted(STALKER_SUCK))]
        }
        __c0 if __c0 == V::Monster::GAS_BOMB => {
            vec![(V::Power::MINION_POWER, PowerAmount::Counted(1))]
        }
        // `Sleep`: Plating, then Asleep.
        __c0 if __c0 == V::Monster::LAGAVULIN_MATRIARCH => vec![
            (
                V::Power::PLATING_POWER,
                PowerAmount::Counted(MATRIARCH_PLATING),
            ),
            (
                V::Power::ASLEEP_POWER,
                PowerAmount::Counted(MATRIARCH_ASLEEP),
            ),
        ],
        __c0 if __c0 == V::Monster::PHANTASMAL_GARDENER => vec![(
            V::Power::SKITTISH_POWER,
            PowerAmount::Counted(match ascension.has(AscensionLevel::ToughEnemies) {
                true => GARDENER_SKITTISH_TOUGH,
                false => GARDENER_SKITTISH,
            }),
        )],
        __c0 if __c0 == V::Monster::SKULKING_COLONY => vec![(
            V::Power::HARDENED_SHELL_POWER,
            PowerAmount::Counted(COLONY_SHELL),
        )],
        __c0 if __c0 == V::Monster::TERROR_EEL => vec![(
            V::Power::SHRIEK_POWER,
            PowerAmount::Counted(match ascension.has(AscensionLevel::ToughEnemies) {
                true => EEL_SHRIEK_TOUGH,
                false => EEL_SHRIEK,
            }),
        )],
        _ => return None,
    })
}

/// Which branch an Underdocks monster's conditional state takes.
pub(super) fn conditional<V: crate::Build>(
    monster: V::Monster,
    at: u8,
    spawn: MonsterSpawn<V::Encounter>,
) -> Result<u8, ContentError> {
    match monster {
        __c0 if __c0 == V::Monster::TOADPOLE => toadpole_opens::<V>(at, spawn),
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
fn toadpole_opens<V: crate::Build>(
    at: u8,
    spawn: MonsterSpawn<V::Encounter>,
) -> Result<u8, ContentError> {
    let state =
        |wanted| crate::behaviour::monsters::state_of::<V>(V::Monster::TOADPOLE, wanted, at);
    match (
        spawn.encounter,
        spawn.roster_size.get(),
        spawn.position.get(),
    ) {
        (__c0, 2, 0) if __c0 == V::Encounter::TOADPOLES_WEAK => state(V::MoveId::SPIKEN_MOVE),
        (__c0, 2, 1) if __c0 == V::Encounter::TOADPOLES_WEAK => state(V::MoveId::WHIRL_MOVE),
        _ => unimplemented!("a Toadpole spawned {spawn:?}"),
    }
}

/// Perform one of an Underdocks monster's moves, where the generator left it
/// to a hand port.
pub(super) fn perform<V: crate::Build, F: Fight<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    step: u8,
    fight: &mut F,
) -> Option<u8> {
    match (monster, move_id) {
        // `GimmeMove`: the hits, then each Thievery steals.
        (__c0, __c1) if (__c0 == V::Monster::GREMLIN_MERC && __c1 == V::MoveId::GIMME_MOVE) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            steal::<V, _>(fight);
        }
        // `DoubleSmashMove`: the hits, the steal, then Weak on each target.
        (__c0, __c1)
            if (__c0 == V::Monster::GREMLIN_MERC && __c1 == V::MoveId::DOUBLE_SMASH_MOVE) =>
        {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            steal::<V, _>(fight);
            fight.apply_to_opponents(V::Power::WEAK_POWER, PowerAmount::Counted(MERC_WEAK));
        }
        // `HeheMove`: the hit, the steal, then Strength on itself.
        (__c0, __c1) if (__c0 == V::Monster::GREMLIN_MERC && __c1 == V::MoveId::HEHE_MOVE) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            steal::<V, _>(fight);
            fight.apply_to_actor(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(MERC_STRENGTH),
            );
        }
        // `SpawnedMove`: it wakes, which is an animation and `IsAwake`, which
        // nothing reads.
        (__c0, __c1)
            if ((__c0 == V::Monster::FAT_GREMLIN || __c0 == V::Monster::SNEAKY_GREMLIN)
                && __c1 == V::MoveId::SPAWNED_MOVE) => {}
        // `FleeMove`: `CreatureCmd.Escape` on itself.
        (__c0, __c1) if (__c0 == V::Monster::FAT_GREMLIN && __c1 == V::MoveId::FLEE_MOVE) => {
            fight.escape()
        }
        // `DarkStrikeMove`: its hit. `BeforeDamage` plays its sound alone.
        (__c0, __c1)
            if ((__c0 == V::Monster::CALCIFIED_CULTIST || __c0 == V::Monster::DAMP_CULTIST)
                && __c1 == V::MoveId::DARK_STRIKE_MOVE) =>
        {
            fight.attack_opponents()
        }
        // `ScratchMove` and `DiseaseBiteMove`: a turn off the wait for a
        // summon, then the hit.
        (__c0, __c1)
            if (__c0 == V::Monster::TWO_TAILED_RAT
                && (__c1 == V::MoveId::SCRATCH_MOVE || __c1 == V::MoveId::DISEASE_BITE_MOVE)) =>
        {
            rat_waits::<V, _>(fight);
            fight.attack_opponents();
        }
        (__c0, __c1) if (__c0 == V::Monster::TWO_TAILED_RAT && __c1 == V::MoveId::SCREECH_MOVE) => {
            rat_waits::<V, _>(fight);
            fight.apply_to_opponents(V::Power::FRAIL_POWER, PowerAmount::Counted(SCREECH_FRAIL));
        }
        (__c0, __c1)
            if (__c0 == V::Monster::TWO_TAILED_RAT && __c1 == V::MoveId::CALL_FOR_BACKUP_MOVE) =>
        {
            call_for_backup::<V, _>(fight)
        }
        // `PressurizeMove`: Steam Eruption on itself.
        (__c0, __c1)
            if (__c0 == V::Monster::WATERFALL_GIANT && __c1 == V::MoveId::PRESSURIZE_MOVE) =>
        {
            let steam = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => 20,
                false => 15,
            };
            fight.apply_to_actor(V::Power::STEAM_ERUPTION_POWER, PowerAmount::Counted(steam));
        }
        // `RamMove` and `PressureUpMove`: the hit, then three Steam Eruption.
        (__c0, __c1)
            if (__c0 == V::Monster::WATERFALL_GIANT
                && (__c1 == V::MoveId::RAM_MOVE || __c1 == V::MoveId::PRESSURE_UP_MOVE)) =>
        {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.apply_to_actor(
                V::Power::STEAM_ERUPTION_POWER,
                PowerAmount::Counted(GIANT_STEAM),
            );
        }
        // `StompMove`: the hit, a Weak on each target, then three Steam
        // Eruption.
        (__c0, __c1) if (__c0 == V::Monster::WATERFALL_GIANT && __c1 == V::MoveId::STOMP_MOVE) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(V::Power::WEAK_POWER, PowerAmount::Counted(1));
            fight.apply_to_actor(
                V::Power::STEAM_ERUPTION_POWER,
                PowerAmount::Counted(GIANT_STEAM),
            );
        }
        // `SiphonMove`: `SiphonHeal` for each player, of which a fight here
        // has one, then three Steam Eruption.
        (__c0, __c1) if (__c0 == V::Monster::WATERFALL_GIANT && __c1 == V::MoveId::SIPHON_MOVE) => {
            let heal = match fight.ascension().has(AscensionLevel::ToughEnemies) {
                true => 15,
                false => 10,
            };
            let giant = fight.actor();
            fight.heal(giant, heal);
            fight.apply_to_actor(
                V::Power::STEAM_ERUPTION_POWER,
                PowerAmount::Counted(GIANT_STEAM),
            );
        }
        // `PressureGunMove`: the hit, five more on the next, then three Steam
        // Eruption.
        (__c0, __c1)
            if (__c0 == V::Monster::WATERFALL_GIANT && __c1 == V::MoveId::PRESSURE_GUN_MOVE) =>
        {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.model(|model| {
                let crate::behaviour::monsters::Model::WaterfallGiant { gun_raises, .. } = model
                else {
                    unreachable!("a Waterfall Giant's model is its own")
                };
                *gun_raises += 1;
            });
            fight.apply_to_actor(
                V::Power::STEAM_ERUPTION_POWER,
                PowerAmount::Counted(GIANT_STEAM),
            );
        }
        // `AboutToBlowMove`: the Steam Eruption it built becomes the damage
        // its Explode deals, and comes off.
        (__c0, __c1)
            if (__c0 == V::Monster::WATERFALL_GIANT && __c1 == V::MoveId::ABOUT_TO_BLOW_MOVE) =>
        {
            use sts2_core::CreatureView as _;

            let giant = fight.actor();
            let built = fight
                .creatures()
                .find(|creature| creature.id() == giant)
                .and_then(|creature| creature.power(V::Power::STEAM_ERUPTION_POWER))
                .map_or(0, |amount| match amount {
                    PowerAmount::Counted(amount) => amount,
                    PowerAmount::Endless => unimplemented!("an endless Steam Eruption"),
                });
            fight.model(|model| {
                let crate::behaviour::monsters::Model::WaterfallGiant { eruption, .. } = model
                else {
                    unreachable!("a Waterfall Giant's model is its own")
                };
                *eruption = built;
            });
            fight.remove_power(giant, V::Power::STEAM_ERUPTION_POWER);
        }
        // `ExplodeMove`: the hit, then `CreatureCmd.Kill` on itself.
        (__c0, __c1)
            if (__c0 == V::Monster::WATERFALL_GIANT && __c1 == V::MoveId::EXPLODE_MOVE) =>
        {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            let giant = fight.actor();
            fight.kill(giant);
        }
        // `BeckonMove`: for the one player, a Beckon into the draw pile at
        // random, then one into the discard pile.
        (__c0, __c1) if (__c0 == V::Monster::SOUL_FYSH && __c1 == V::MoveId::BECKON_MOVE) => {
            fight.add_at_random(beckon::<V>(), sts2_core::Pile::Draw);
            fight.add_generated_cards(
                sts2_core::CardIdent::new(beckon::<V>()),
                sts2_core::Pile::Discard,
                1,
            );
        }
        // `GazeMove`: its hit, then a Beckon into the discard pile.
        (__c0, __c1) if (__c0 == V::Monster::SOUL_FYSH && __c1 == V::MoveId::GAZE_MOVE) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.add_generated_cards(
                sts2_core::CardIdent::new(beckon::<V>()),
                sts2_core::Pile::Discard,
                1,
            );
        }
        // `FadeMove`: two Intangible. `IsInvisible` and the music are on
        // screen alone.
        (__c0, __c1) if (__c0 == V::Monster::SOUL_FYSH && __c1 == V::MoveId::FADE_MOVE) => {
            fight.apply_to_actor(V::Power::INTANGIBLE_POWER, PowerAmount::Counted(FYSH_FADE));
        }
        // `ScreamMove`: its hit, then three Vulnerable on each target.
        (__c0, __c1) if (__c0 == V::Monster::SOUL_FYSH && __c1 == V::MoveId::SCREAM_MOVE) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(
                V::Power::VULNERABLE_POWER,
                PowerAmount::Counted(FYSH_SCREAM),
            );
        }
        // The Matriarch's four waking moves. Each first stops its sleeping
        // effect, which is all on screen.
        (__c0, __c1)
            if (__c0 == V::Monster::LAGAVULIN_MATRIARCH
                && (__c1 == V::MoveId::SLASH_MOVE || __c1 == V::MoveId::DISEMBOWEL_MOVE)) =>
        {
            fight.attack_opponents()
        }
        // `Slash2Move`: its hit, then powered block on itself.
        (__c0, __c1)
            if (__c0 == V::Monster::LAGAVULIN_MATRIARCH && __c1 == V::MoveId::SLASH2_MOVE) =>
        {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
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
        (__c0, __c1)
            if (__c0 == V::Monster::LAGAVULIN_MATRIARCH && __c1 == V::MoveId::SOUL_SIPHON_MOVE) =>
        {
            fight.apply_to_opponents(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(-MATRIARCH_SIPHON),
            );
            fight.apply_to_opponents(
                V::Power::DEXTERITY_POWER,
                PowerAmount::Counted(-MATRIARCH_SIPHON),
            );
            fight.apply_to_actor(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(MATRIARCH_SIPHON),
            );
        }
        // `EnlargeMove`: Strength on itself. `EnlargeTriggers` and the scale
        // it sets size it on screen alone.
        (__c0, __c1)
            if (__c0 == V::Monster::PHANTASMAL_GARDENER && __c1 == V::MoveId::ENLARGE_MOVE) =>
        {
            let strength = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => GARDENER_ENLARGE_DEADLY,
                false => GARDENER_ENLARGE,
            };
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(strength));
        }
        // `BloatMove`: a Gas Bomb into the first slot no enemy holds, for each
        // of its `BloatAmount`, which stays at one. Then its hit.
        (__c0, __c1) if (__c0 == V::Monster::LIVING_FOG && __c1 == V::MoveId::BLOAT_MOVE) => {
            fight.summon_into_first_free_slot(V::Monster::GAS_BOMB);
            fight.attack_opponents();
        }
        // `ExplodeMove`: its hit, then `CreatureCmd.Kill` on itself.
        // `HasExploded` picks how it dies on screen alone.
        (__c0, __c1) if (__c0 == V::Monster::GAS_BOMB && __c1 == V::MoveId::EXPLODE_MOVE) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            let bomb = fight.actor();
            fight.kill(bomb);
        }
        // `SpikeSpitMove`: the Thorns Spiken gave, taken back, then its hits.
        (__c0, __c1) if (__c0 == V::Monster::TOADPOLE && __c1 == V::MoveId::SPIKE_SPIT_MOVE) => {
            fight.apply_to_actor(V::Power::THORNS_POWER, PowerAmount::Counted(-SPIKEN));
            fight.attack_opponents();
        }
        (monster, move_id) => {
            return V::generated_move(monster, move_id, step, fight);
        }
    }
    None
}

/// `TurnsUntilSummonable--`.
/// `ThieveryPower.Steal` for the Gremlin Merc's one instance: the lesser of
/// its stack and the player's gold, where the player stands and holds any.
fn steal<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    use sts2_core::CreatureView as _;

    if fight.opponents().is_empty() {
        return;
    }
    let merc = fight.actor();
    let Some(PowerAmount::Counted(amount)) = fight
        .creatures()
        .find(|creature| creature.id() == merc)
        .and_then(|creature| creature.power(V::Power::THIEVERY_POWER))
    else {
        return;
    };
    let gold = fight.gold();
    if gold <= 0 {
        return;
    }
    let taken = amount.min(gold);
    fight.lose_gold(taken);
    fight.write_power(merc, V::Power::THIEVERY_POWER, |state| {
        *state.stolen_gold() += taken
    });
}

fn rat_waits<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.model(|model| {
        let crate::behaviour::monsters::Model::TwoTailedRat { summonable_in, .. } = model else {
            unreachable!("a Two-Tailed Rat's model is its own")
        };
        *summonable_in = summonable_in.saturating_sub(1);
    });
}

/// `CallForBackup`: a rat into the last slot no enemy holds. Then every rat,
/// the new one included, counts one past the highest count any of them held.
fn call_for_backup<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.summon_into_last_free_slot(V::Monster::TWO_TAILED_RAT);
    let mut highest = 0;
    fight.enemy_models(|model| {
        if let crate::behaviour::monsters::Model::TwoTailedRat { backups, .. } = *model {
            highest = highest.max(backups + 1);
        }
    });
    fight.enemy_models(|model| {
        if let crate::behaviour::monsters::Model::TwoTailedRat { backups, .. } = model {
            *backups = highest;
        }
    });
}

/// A Two-Tailed Rat's `RAND` branches, in the order it adds them. Scratch,
/// Disease Bite and Screech weigh 1 while it cannot summon and 1/12 while it
/// can. Call for Backup weighs 0.75 while it can, and nothing otherwise.
pub(super) fn rat_weight<V: crate::Build>(
    at: usize,
    spawn: MonsterSpawn<V::Encounter>,
    model: &crate::behaviour::monsters::Model<V>,
    advertised: impl Fn(V::MoveId) -> bool,
) -> f32 {
    let crate::behaviour::monsters::Model::TwoTailedRat {
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
        && !advertised(V::MoveId::CALL_FOR_BACKUP_MOVE);
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
pub(super) fn computed_damage<V: crate::Build>(
    monster: V::Monster,
    move_id: V::MoveId,
    ascension: u8,
    model: &crate::behaviour::monsters::Model<V>,
) -> i32 {
    let deadly = ascension >= AscensionLevel::DeadlyEnemies as u8;
    match (monster, move_id) {
        // `CurrentPressureGunDamage`: its base, and five for each Pressure
        // Gun since.
        (__c0, __c1)
            if (__c0 == V::Monster::WATERFALL_GIANT && __c1 == V::MoveId::PRESSURE_GUN_MOVE) =>
        {
            let crate::behaviour::monsters::Model::WaterfallGiant { gun_raises, .. } = *model
            else {
                unreachable!("a Waterfall Giant's model is its own")
            };
            (if deadly { 23 } else { 20 }) + GIANT_GUN_RAISE * i32::from(gun_raises)
        }
        // `SteamEruptionDamage`, which About to Blow saved.
        (__c0, __c1)
            if (__c0 == V::Monster::WATERFALL_GIANT && __c1 == V::MoveId::EXPLODE_MOVE) =>
        {
            let crate::behaviour::monsters::Model::WaterfallGiant { eruption, .. } = *model else {
                unreachable!("a Waterfall Giant's model is its own")
            };
            eruption
        }
        (__c0, __c1) if (__c0 == V::Monster::GAS_BOMB && __c1 == V::MoveId::EXPLODE_MOVE) => {
            match deadly {
                true => 9,
                false => 8,
            }
        }
        (monster, move_id) => unimplemented!("the damage {monster:?} works out for {move_id:?}"),
    }
}

/// The Matriarch's `SLEEP_BRANCH`: asleep while it holds Asleep, and slashing
/// once it does not.
pub(super) fn matriarch_branch<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    at: u8,
    powers: &[sts2_core::Listener<'_, V, P>],
) -> Result<u8, ContentError> {
    let asleep = powers.iter().any(|listener| {
        matches!(listener, sts2_core::Listener::Power { power: __c0, .. } if *__c0 == V::Power::ASLEEP_POWER)
    });
    let wanted = match asleep {
        true => V::MoveId::SLEEP_MOVE,
        false => V::MoveId::SLASH_MOVE,
    };
    crate::behaviour::monsters::state_of::<V>(V::Monster::LAGAVULIN_MATRIARCH, wanted, at)
}
