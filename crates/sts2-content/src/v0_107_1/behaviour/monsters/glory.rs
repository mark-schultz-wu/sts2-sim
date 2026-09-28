//! The third act's monsters.

use sts2_core::{Fight, PowerAmount};

use super::super::super::generated::ascension::AscensionLevel;
use super::super::super::generated::monster_moves;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::super::super::generated::powers::Power;
use super::{Number, V};

/// Powers a Glory monster applies to itself as it enters combat.
pub(super) fn starting_powers(
    monster: Monster,
    ascension: AscensionLevel,
) -> Option<Vec<(Power, PowerAmount<Number>)>> {
    Some(match monster {
        // `_buffAmt`, which is also Paper Cuts'.
        Monster::ScrollOfBiting => vec![(Power::PaperCutsPower, PowerAmount::Counted(2))],
        Monster::LivingShield => vec![(Power::RampartPower, PowerAmount::Counted(25))],
        Monster::GlobeHead => vec![(Power::GalvanicPower, PowerAmount::Counted(6))],
        Monster::TheLost => vec![(Power::PossessStrengthPower, PowerAmount::Counted(1))],
        Monster::TheForgotten => vec![(Power::PossessSpeedPower, PowerAmount::Counted(1))],
        // Its `StartingHpReduction` is nothing outside the Punch-Off event's
        // encounter, which rolls it.
        Monster::PunchConstruct => vec![(Power::ArtifactPower, PowerAmount::Counted(1))],
        Monster::FrogKnight => {
            let plating = match ascension.has(AscensionLevel::ToughEnemies) {
                true => 19,
                false => 15,
            };
            vec![(Power::PlatingPower, PowerAmount::Counted(plating))]
        }
        _ => return None,
    })
}

/// Which move a Glory monster selects from a conditional state.
pub(super) fn conditional(
    monster: Monster,
    at: u8,
    spawn: sts2_core::MonsterSpawn<super::super::super::generated::encounters::Encounter>,
    model: &super::Model,
) -> Result<u8, sts2_core::ContentError> {
    let wanted = match monster {
        // `SHIELD_SLAM_BRANCH`: Shield Slam again while an ally stands, and
        // Smash once none does.
        Monster::LivingShield => match spawn.standing > 1 {
            true => MoveId::ShieldSlamMove,
            false => MoveId::SmashMove,
        },
        // `HALF_HEALTH`: Beetle Charge once, below half its maximum.
        Monster::FrogKnight => {
            let super::Model::FrogKnight { beetle_charged } = *model else {
                unreachable!("a Frog Knight holding {model:?}")
            };
            match beetle_charged || spawn.hp >= spawn.max_hp / 2 {
                true => MoveId::TongueLash,
                false => MoveId::BeetleCharge,
            }
        }
        monster => unimplemented!("what {monster:?} asks the fight at state {at}"),
    };
    super::state_of(monster, wanted, at)
}

/// `DebilitatingSmogStrengthStealAmount` and
/// `DebilitatingSmogDexStealAmount`, two at every ascension.
const STOLEN: Number = 2;

/// Miasma's block.
const MIASMA_BLOCK: Number = 8;

/// `DevotedSculptor`'s `_ritualGain`.
const RITUAL: Number = 9;

/// Perform one of the Glory monsters' moves, where the generator left it to
/// a hand port.
pub(super) fn perform<F: Fight<V>>(
    monster: Monster,
    move_id: MoveId,
    model: &mut super::Model,
    fight: &mut F,
) {
    match (monster, move_id) {
        // `BeetleChargeMove`: it marks the charge, then hits.
        (Monster::FrogKnight, MoveId::BeetleCharge) => {
            let super::Model::FrogKnight { beetle_charged } = model else {
                unreachable!("a Frog Knight holding {model:?}")
            };
            *beetle_charged = true;
            fight.attack_opponents();
        }
        // `ForbiddenIncantationMove`: Ritual on itself, from nobody.
        (Monster::DevotedSculptor, MoveId::ForbiddenIncantationMove) => {
            let actor = fight.actor();
            fight.apply_power_from(
                actor,
                Power::RitualPower,
                PowerAmount::Counted(RITUAL),
                None,
            );
        }
        // `DebilitatingSmogMove`: two Strength off each target, then two
        // onto itself.
        (Monster::TheLost, MoveId::DebilitatingSmog) => {
            fight.apply_to_opponents(Power::StrengthPower, PowerAmount::Counted(-STOLEN));
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(STOLEN));
        }
        // `MiasmaMove`: two Dexterity off each target, eight block, then two
        // Dexterity onto itself.
        (Monster::TheForgotten, MoveId::Miasma) => {
            use sts2_core::{Amount, Effect, Scaling};

            fight.apply_to_opponents(Power::DexterityPower, PowerAmount::Counted(-STOLEN));
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(MIASMA_BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            fight.apply_to_actor(Power::DexterityPower, PowerAmount::Counted(STOLEN));
        }
        (Monster::TheForgotten, MoveId::Dread) => fight.attack_opponents(),
        _ => monster_moves::perform(monster, move_id, fight),
    }
}

/// A Glory monster's attack damage that it works out from the fight.
pub(super) fn computed_damage<P: sts2_core::Printed<V> + ?Sized>(
    monster: Monster,
    move_id: MoveId,
    ascension: u8,
    powers: &[sts2_core::Listener<'_, V, P>],
) -> i32 {
    let held = |wanted| {
        powers
            .iter()
            .find_map(|listener| match listener {
                sts2_core::Listener::Power { power, amount, .. } if *power == wanted => {
                    Some(*amount)
                }
                _ => None,
            })
            .map_or(0, |amount| match amount {
                PowerAmount::Counted(amount) => amount,
                PowerAmount::Endless => unimplemented!("an endless {wanted:?}"),
            })
    };
    let deadly = ascension >= AscensionLevel::DeadlyEnemies as u8;
    match (monster, move_id) {
        // `DreadDamage`: its base, and its own Dexterity.
        (Monster::TheForgotten, MoveId::Dread) => {
            (if deadly { 15 } else { 13 }) + held(Power::DexterityPower)
        }
        (monster, move_id) => unimplemented!("the damage {monster:?} works out for {move_id:?}"),
    }
}
