//! The third act's monsters.

use sts2_core::{Fight, PowerAmount};

use super::super::super::generated::ascension::AscensionLevel;
use super::super::super::generated::monster_moves;
use super::super::super::generated::monsters::{Monster, MoveId, State};
use super::super::super::generated::powers::Power;
use super::{Number, V};

/// Powers a Glory monster applies to itself as it enters combat.
pub(super) fn starting_powers(
    monster: Monster,
    ascension: AscensionLevel,
    model: &super::Model,
) -> Option<Vec<(Power, PowerAmount<Number>)>> {
    Some(match monster {
        // `StockAmount`, which is two unless a respawn wrote it. Nothing is
        // applied at none.
        Monster::Axebot => match axebot_stock(model) {
            0 => Vec::new(),
            stock => vec![(Power::StockPower, PowerAmount::Counted(Number::from(stock)))],
        },
        // `_buffAmt`, which is also Paper Cuts'.
        Monster::ScrollOfBiting => vec![(Power::PaperCutsPower, PowerAmount::Counted(2))],
        Monster::LivingShield => vec![(Power::RampartPower, PowerAmount::Counted(25))],
        Monster::GlobeHead => vec![(Power::GalvanicPower, PowerAmount::Counted(6))],
        Monster::MechaKnight => vec![(Power::ArtifactPower, PowerAmount::Counted(3))],
        // It listens for its own death to clear its animation, which touches
        // nothing in the fight.
        Monster::SoulNexus => Vec::new(),
        Monster::Zapbot => vec![(Power::HighVoltagePower, PowerAmount::Counted(2))],
        // Each calls the base alone, and places itself where the Fabricator's
        // bots fall.
        Monster::Stabbot | Monster::Guardbot | Monster::Noisebot => Vec::new(),
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
        // `fabricateBranch`: its random choice while fewer than four stand on
        // its side, itself included, and Disintegrate otherwise.
        Monster::Fabricator => {
            let State::Conditional(&[fabricate, disintegrate]) =
                sts2_core::Table::data(monster).states[usize::from(at)]
            else {
                unreachable!("the Fabricator's branch is its two-way conditional")
            };
            return Ok(match spawn.standing < FABRICATES_BELOW {
                true => fabricate,
                false => disintegrate,
            });
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

/// `Axebot.StockAmount`.
fn axebot_stock(model: &super::Model) -> u8 {
    let super::Model::Axebot { stock } = *model else {
        unreachable!("an Axebot holding {model:?}")
    };
    stock.unwrap_or(2)
}

/// `Fabricator.defenseSpawns`, in the order the set lists them.
const DEFENSE_SPAWNS: [Monster; 2] = [Monster::Guardbot, Monster::Noisebot];

/// `Fabricator.aggroSpawns`.
const AGGRO_SPAWNS: [Monster; 2] = [Monster::Zapbot, Monster::Stabbot];

/// `CanFabricate`: fewer than this many stand on its side.
const FABRICATES_BELOW: u8 = 4;

/// The block Guard gives each Fabricator.
const GUARD_BLOCK: Number = 15;

/// `SpawnBot`: one of `options` other than the last it spawned, drawn off
/// `MonsterAi`, into the encounter's next slot, with Minion from the
/// Fabricator.
fn spawn_bot<F: Fight<V>>(fight: &mut F, model: &mut super::Model, options: &[Monster]) {
    let super::Model::Fabricator { last } = model else {
        unreachable!("a Fabricator holding {model:?}")
    };
    let items: Vec<Monster> = options
        .iter()
        .copied()
        .filter(|&option| Some(option) != *last)
        .collect();
    let Some(bot) = fight.monster_ai_choose(&items) else {
        return;
    };
    *last = Some(bot);
    if let Some(spawned) = fight.summon_into_first_free_slot(bot) {
        fight.apply_power(spawned, Power::MinionPower, PowerAmount::Counted(1));
    }
}

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
        // `BootUpMove`: its block, then Strength for each stock it lost.
        (Monster::Axebot, MoveId::BootUpMove) => {
            use sts2_core::{Amount, Effect, Scaling};

            let deadly = fight.ascension().has(AscensionLevel::DeadlyEnemies);
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(if deadly { 15 } else { 10 }),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            let strength = if deadly { 4 } else { 3 } * (2 - Number::from(axebot_stock(model)));
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(strength));
        }
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
        // `FabricateMove`: a defensive bot, then an aggressive one.
        (Monster::Fabricator, MoveId::FabricateMove) => {
            spawn_bot(fight, model, &DEFENSE_SPAWNS);
            spawn_bot(fight, model, &AGGRO_SPAWNS);
        }
        // `FabricatingStrikeMove`: its hit, then an aggressive bot.
        (Monster::Fabricator, MoveId::FabricatingStrikeMove) => {
            fight.attack_opponents();
            spawn_bot(fight, model, &AGGRO_SPAWNS);
        }
        // `GuardMove`: fifteen block, unpowered, on each Fabricator.
        (Monster::Guardbot, MoveId::GuardMove) => {
            use sts2_core::{Amount, CreatureView as _, Effect, Scaling};

            let fabricators: Vec<_> = fight
                .creatures()
                .filter(|creature| creature.monster() == Some(Monster::Fabricator))
                .map(|creature| creature.id())
                .collect();
            for fabricator in fabricators {
                let block = fight.resolve(Effect::GainBlock {
                    recipient: fabricator,
                    amount: Amount::Number(GUARD_BLOCK),
                    scaling: Scaling::Static,
                });
                fight.apply(block);
            }
        }
        // `NoiseMove`: for each target, a Dazed into the discard pile, then
        // one into the draw pile at random. A fight here has one player.
        (Monster::Noisebot, MoveId::NoiseMove) => {
            use super::super::super::generated::cards::{Cards, StatusCard};

            let dazed = Cards::Status(StatusCard::Dazed);
            fight.add_generated_cards(
                sts2_core::CardIdent::new(dazed),
                sts2_core::Pile::Discard,
                1,
            );
            fight.add_at_random(dazed, sts2_core::Pile::Draw);
        }
        // `FlamethrowerMove`: four Burns into the hand of the one player.
        (Monster::MechaKnight, MoveId::FlamethrowerMove) => {
            use super::super::super::generated::cards::{Cards, StatusCard};

            fight.add_generated_cards(
                sts2_core::CardIdent::new(Cards::Status(StatusCard::Burn)),
                sts2_core::Pile::Hand,
                4,
            );
        }
        // `WindupMove`: fifteen block and five Strength. `IsWoundUp` picks its
        // animations alone.
        (Monster::MechaKnight, MoveId::WindupMove) => {
            use sts2_core::{Amount, Effect, Scaling};

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(15),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(5));
        }
        (Monster::MechaKnight, MoveId::HeavyCleaveMove) => fight.attack_opponents(),
        // `HexMove`: two Hex on each target.
        (Monster::SpectralKnight, MoveId::Hex) => {
            fight.apply_to_opponents(Power::HexPower, PowerAmount::Counted(2));
        }
        // `MagicBombMove`: its hit. The rest picks where its effect lands.
        (Monster::MagiKnight, MoveId::MagicBomb) => fight.attack_opponents(),
        // `JudicialFlightMove`: Soar on itself. `IsFlying` picks its sounds
        // alone.
        (Monster::OwlMagistrate, MoveId::JudicialFlight) => {
            fight.apply_to_actor(Power::SoarPower, PowerAmount::Counted(1));
        }
        // `VerdictMove`: its hit, four Vulnerable on each target, then Soar
        // comes off.
        (Monster::OwlMagistrate, MoveId::Verdict) => {
            fight.attack_opponents();
            fight.apply_to_opponents(Power::VulnerablePower, PowerAmount::Counted(4));
            let actor = fight.actor();
            fight.remove_power(actor, Power::SoarPower);
        }
        // `LeechingHugMove`: three Weak on each target from nobody, then
        // three Strength onto itself.
        (Monster::SlimedBerserker, MoveId::LeechingHugMove) => {
            for target in fight.opponents() {
                fight.apply_power_from(target, Power::WeakPower, PowerAmount::Counted(3), None);
            }
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(3));
        }
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
