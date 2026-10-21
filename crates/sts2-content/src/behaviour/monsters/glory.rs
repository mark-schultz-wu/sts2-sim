//! The third act's monsters.

use sts2_core::{Fight, PowerAmount};

use crate::behaviour::monsters::Number;
use crate::generated::ascension::AscensionLevel;
use crate::ids::{CardIds as _, CardVarIds as _, MonsterIds as _, MoveIds as _, PowerIds as _};

/// Powers a Glory monster applies to itself as it enters combat.
pub(super) fn starting_powers<V: crate::Build>(
    monster: V::Monster,
    ascension: AscensionLevel,
    model: &crate::behaviour::monsters::Model<V>,
) -> Option<Vec<(V::Power, PowerAmount<Number>)>> {
    Some(match monster {
        // `StockAmount`, which is two unless a respawn wrote it. Nothing is
        // applied at none.
        __c0 if __c0 == V::Monster::AXEBOT => match axebot_stock::<V>(model) {
            0 => Vec::new(),
            stock => vec![(
                V::Power::STOCK_POWER,
                PowerAmount::Counted(Number::from(stock)),
            )],
        },
        // `_buffAmt`, which is also Paper Cuts'.
        __c0 if __c0 == V::Monster::SCROLL_OF_BITING => {
            vec![(V::Power::PAPER_CUTS_POWER, PowerAmount::Counted(2))]
        }
        __c0 if __c0 == V::Monster::LIVING_SHIELD => {
            vec![(V::Power::RAMPART_POWER, PowerAmount::Counted(25))]
        }
        // `GalvanicPowerAmount`, which a build may raise from Deadly Enemies.
        __c0 if __c0 == V::Monster::GLOBE_HEAD => {
            let amount = match ascension.has(AscensionLevel::DeadlyEnemies) {
                true => V::GLOBE_HEAD_DEADLY_GALVANIC,
                false => 6,
            };
            vec![(V::Power::GALVANIC_POWER, PowerAmount::Counted(amount))]
        }
        __c0 if __c0 == V::Monster::MECHA_KNIGHT => {
            vec![(V::Power::ARTIFACT_POWER, PowerAmount::Counted(3))]
        }
        // It listens for its own death to clear its animation, which touches
        // nothing in the fight.
        __c0 if __c0 == V::Monster::SOUL_NEXUS => Vec::new(),
        __c0 if __c0 == V::Monster::ZAPBOT => {
            vec![(V::Power::HIGH_VOLTAGE_POWER, PowerAmount::Counted(2))]
        }
        // Each calls the base alone, and places itself where the Fabricator's
        // bots fall.
        __c0 if __c0 == V::Monster::STABBOT
            || __c0 == V::Monster::GUARDBOT
            || __c0 == V::Monster::NOISEBOT =>
        {
            Vec::new()
        }
        __c0 if __c0 == V::Monster::THE_LOST => {
            vec![(V::Power::POSSESS_STRENGTH_POWER, PowerAmount::Counted(1))]
        }
        __c0 if __c0 == V::Monster::THE_FORGOTTEN => {
            vec![(V::Power::POSSESS_SPEED_POWER, PowerAmount::Counted(1))]
        }
        // Artifact, then the hit points `hp_after_added` takes off.
        __c0 if __c0 == V::Monster::PUNCH_CONSTRUCT => {
            vec![(V::Power::ARTIFACT_POWER, PowerAmount::Counted(1))]
        }
        // `WitheringPresencePower` on itself for each opponent, which the one
        // player is, then Artifact.
        __c0 if __c0 == V::Monster::AEONGLASS => vec![
            (V::Power::WITHERING_PRESENCE_POWER, PowerAmount::Counted(6)),
            (V::Power::ARTIFACT_POWER, PowerAmount::Counted(3)),
        ],
        __c0 if __c0 == V::Monster::TORCH_HEAD_AMALGAM => {
            vec![(V::Power::MINION_POWER, PowerAmount::Counted(1))]
        }
        // It finds the Amalgam among the enemies, which the fight asks it
        // about as it dies.
        __c0 if __c0 == V::Monster::QUEEN => Vec::new(),
        __c0 if __c0 == V::Monster::TEST_SUBJECT => {
            let enrage = match ascension.has(AscensionLevel::DeadlyEnemies) {
                true => 3,
                false => 2,
            };
            vec![
                (V::Power::ADAPTABLE_POWER, PowerAmount::Counted(1)),
                (V::Power::ENRAGE_POWER, PowerAmount::Counted(enrage)),
            ]
        }
        __c0 if __c0 == V::Monster::FROG_KNIGHT => {
            let plating = match ascension.has(AscensionLevel::ToughEnemies) {
                true => 19,
                false => 15,
            };
            vec![(V::Power::PLATING_POWER, PowerAmount::Counted(plating))]
        }
        _ => return None,
    })
}

/// Which move a Glory monster selects from a conditional state.
pub(super) fn conditional<V: crate::Build>(
    monster: V::Monster,
    at: u8,
    spawn: sts2_core::MonsterSpawn<V::Encounter>,
    model: &crate::behaviour::monsters::Model<V>,
) -> Result<u8, sts2_core::ContentError> {
    let wanted = match monster {
        // `SHIELD_SLAM_BRANCH`: Shield Slam again while an ally stands, and
        // Smash once none does.
        __c0 if __c0 == V::Monster::LIVING_SHIELD => match spawn.standing > 1 {
            true => V::MoveId::SHIELD_SLAM_MOVE,
            false => V::MoveId::SMASH_MOVE,
        },
        // `HALF_HEALTH`: Beetle Charge once, below half its maximum.
        __c0 if __c0 == V::Monster::FROG_KNIGHT => {
            let crate::behaviour::monsters::Model::FrogKnight { beetle_charged } = *model else {
                unreachable!("a Frog Knight holding {model:?}")
            };
            match beetle_charged || spawn.hp >= spawn.max_hp / 2 {
                true => V::MoveId::TONGUE_LASH,
                false => V::MoveId::BEETLE_CHARGE,
            }
        }
        // `fabricateBranch`: its random choice while fewer than four stand on
        // its side, itself included, and Disintegrate otherwise.
        __c0 if __c0 == V::Monster::FABRICATOR => {
            let Some(&[fabricate, disintegrate]) = V::monster_conditional(monster, at) else {
                unreachable!("the Fabricator's branch is its two-way conditional")
            };
            return Ok(match spawn.standing < FABRICATES_BELOW {
                true => fabricate,
                false => disintegrate,
            });
        }
        // `YOURE_MINE_NOW_BRANCH` and `BURN_BRIGHT_FOR_ME_BRANCH`: Burn Bright
        // For Me while the Amalgam stands, and Off With Your Head once it has
        // died.
        __c0 if __c0 == V::Monster::QUEEN => {
            let crate::behaviour::monsters::Model::Queen { amalgam_died } = *model else {
                unreachable!("a Queen holding {model:?}")
            };
            match amalgam_died {
                false => V::MoveId::BURN_BRIGHT_FOR_ME_MOVE,
                true => V::MoveId::OFF_WITH_YOUR_HEAD_MOVE,
            }
        }
        // `REVIVE_BRANCH`: Multi Claw after the first respawn, and Lacerate
        // after the second.
        __c0 if __c0 == V::Monster::TEST_SUBJECT => match test_subject::<V>(model).0 < 2 {
            true => V::MoveId::MULTI_CLAW_MOVE,
            false => V::MoveId::PHASE3_LACERATE_MOVE,
        },
        monster => unimplemented!("what {monster:?} asks the fight at state {at}"),
    };
    crate::behaviour::monsters::state_of::<V>(monster, wanted, at)
}

/// `TestSubject.Respawns` and `ExtraMultiClawCount`.
fn test_subject<V: crate::Build>(model: &crate::behaviour::monsters::Model<V>) -> (u8, u16) {
    let crate::behaviour::monsters::Model::TestSubject {
        respawns,
        extra_claws,
    } = *model
    else {
        unreachable!("a Test Subject holding {model:?}")
    };
    (respawns, extra_claws)
}

/// `TestSubject.BaseMultiClawCount`.
const MULTI_CLAWS: u32 = 3;

/// A Glory monster's hit count that it works out from its own model.
pub(super) fn computed_repeats<V: crate::Build>(
    monster: V::Monster,
    move_id: V::MoveId,
    model: &crate::behaviour::monsters::Model<V>,
) -> u32 {
    match (monster, move_id) {
        // `MultiClawTotalCount`: its base, and one for each Multi Claw before.
        (__c0, __c1)
            if (__c0 == V::Monster::TEST_SUBJECT && __c1 == V::MoveId::MULTI_CLAW_MOVE) =>
        {
            MULTI_CLAWS + u32::from(test_subject::<V>(model).1)
        }
        (monster, move_id) => unimplemented!("the hits {monster:?} works out for {move_id:?}"),
    }
}

/// `Wither.FakeUpgrade`'s raise to its damage.
const WITHER_RAISE: Number = 3;

/// `Aeonglass.AfterCardGeneratedForCombat`: a Wither made in the fight takes
/// every fake upgrade the Aeonglass has given the ones before it.
pub(super) fn wither_generated<V: crate::Build>(
    model: &crate::behaviour::monsters::Model<V>,
    at: sts2_core::CardAt<V>,
    card: V::Card,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let crate::behaviour::monsters::Model::Aeonglass {
        wither_upgrades, ..
    } = *model
    else {
        unreachable!("an Aeonglass holding {model:?}")
    };
    if card == V::Card::WITHER && wither_upgrades > 0 {
        asked.push(sts2_core::HookEffect::ModifyCards {
            to: sts2_core::Writing::One(at),
            change: sts2_core::Change::Raise(
                V::CardVar::DAMAGE,
                WITHER_RAISE * Number::from(wither_upgrades),
            ),
        });
    }
}

/// `Queen.AfterDeath` for the Amalgam: it has died, and a Queen about to
/// burn it brighter enrages instead.
pub(super) fn amalgam_died<V: crate::Build>(
    model: &mut crate::behaviour::monsters::Model<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let crate::behaviour::monsters::Model::Queen { amalgam_died } = model else {
        unreachable!("a Queen holding {model:?}")
    };
    *amalgam_died = true;
    asked.push(sts2_core::HookEffect::EnterMoveFrom {
        from: V::MoveId::BURN_BRIGHT_FOR_ME_MOVE,
        to: V::MoveId::ENRAGE_MOVE,
    });
}

/// `TestSubject.Revive`: `CreatureCmd.SetMaxHp`, then a heal of as much.
/// A fight here has one player, so the health is not scaled.
fn revive_at<V: crate::Build, F: Fight<V>>(fight: &mut F, hp: Number) {
    let actor = fight.actor();
    fight.set_max_hp(actor, hp);
    fight.heal(actor, hp);
}

/// `DebilitatingSmogStrengthStealAmount` and
/// `DebilitatingSmogDexStealAmount`, two at every ascension.
const STOLEN: Number = 2;

/// Miasma's block.
const MIASMA_BLOCK: Number = 8;

/// `Axebot.StockAmount`.
fn axebot_stock<V: crate::Build>(model: &crate::behaviour::monsters::Model<V>) -> u8 {
    let crate::behaviour::monsters::Model::Axebot { stock } = *model else {
        unreachable!("an Axebot holding {model:?}")
    };
    stock.unwrap_or(2)
}

/// `CanFabricate`: fewer than this many stand on its side.
const FABRICATES_BELOW: u8 = 4;

/// The block Guard gives each Fabricator.
const GUARD_BLOCK: Number = 15;

/// `SpawnBot`: one of `options` other than the last it spawned, drawn off
/// `MonsterAi`, into the encounter's next slot, with Minion from the
/// Fabricator.
fn spawn_bot<V: crate::Build, F: Fight<V>>(fight: &mut F, options: &[V::Monster]) {
    let last = fight.model(|model| {
        let crate::behaviour::monsters::Model::Fabricator { last } = *model else {
            unreachable!("a Fabricator holding {model:?}")
        };
        last
    });
    let items: Vec<V::Monster> = options
        .iter()
        .copied()
        .filter(|&option| Some(option) != last)
        .collect();
    let Some(bot) = fight.monster_ai_choose(&items) else {
        return;
    };
    fight.model(|model| *model = crate::behaviour::monsters::Model::Fabricator { last: Some(bot) });
    if let Some(spawned) = fight.summon_into_first_free_slot(bot) {
        fight.apply_power(spawned, V::Power::MINION_POWER, PowerAmount::Counted(1));
    }
}

/// `DevotedSculptor`'s `_ritualGain`.
const RITUAL: Number = 9;

/// Perform one of the Glory monsters' moves, where the generator left it to
/// a hand port.
pub(super) fn perform<V: crate::Build, F: Fight<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    step: u8,
    fight: &mut F,
) -> Option<u8> {
    match (monster, move_id) {
        // `BootUpMove`: its block, then Strength for each stock it lost.
        (__c0, __c1) if (__c0 == V::Monster::AXEBOT && __c1 == V::MoveId::BOOT_UP_MOVE) => {
            use sts2_core::{Amount, Effect, Scaling};

            let deadly = fight.ascension().has(AscensionLevel::DeadlyEnemies);
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(if deadly { 15 } else { 10 }),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            let stock = fight.model(|model| axebot_stock::<V>(model));
            let strength = if deadly { 4 } else { 3 } * (2 - Number::from(stock));
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(strength));
        }
        // `BeetleChargeMove`: it marks the charge, then hits.
        (__c0, __c1) if (__c0 == V::Monster::FROG_KNIGHT && __c1 == V::MoveId::BEETLE_CHARGE) => {
            fight.model(|model| {
                let crate::behaviour::monsters::Model::FrogKnight { beetle_charged } = model else {
                    unreachable!("a Frog Knight holding {model:?}")
                };
                *beetle_charged = true;
            });
            fight.attack_opponents();
        }
        // `ForbiddenIncantationMove`: Ritual on itself, from nobody.
        (__c0, __c1)
            if (__c0 == V::Monster::DEVOTED_SCULPTOR
                && __c1 == V::MoveId::FORBIDDEN_INCANTATION_MOVE) =>
        {
            let actor = fight.actor();
            fight.apply_power_from(
                actor,
                V::Power::RITUAL_POWER,
                PowerAmount::Counted(RITUAL),
                None,
            );
        }
        // `DebilitatingSmogMove`: two Strength off each target, then two
        // onto itself.
        (__c0, __c1) if (__c0 == V::Monster::THE_LOST && __c1 == V::MoveId::DEBILITATING_SMOG) => {
            fight.apply_to_opponents(V::Power::STRENGTH_POWER, PowerAmount::Counted(-STOLEN));
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(STOLEN));
        }
        // `MiasmaMove`: two Dexterity off each target, eight block, then two
        // Dexterity onto itself.
        (__c0, __c1) if (__c0 == V::Monster::THE_FORGOTTEN && __c1 == V::MoveId::MIASMA) => {
            use sts2_core::{Amount, Effect, Scaling};

            fight.apply_to_opponents(V::Power::DEXTERITY_POWER, PowerAmount::Counted(-STOLEN));
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(MIASMA_BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            fight.apply_to_actor(V::Power::DEXTERITY_POWER, PowerAmount::Counted(STOLEN));
        }
        (__c0, __c1) if (__c0 == V::Monster::THE_FORGOTTEN && __c1 == V::MoveId::DREAD) => {
            fight.attack_opponents()
        }
        // `FabricateMove`: a defensive bot, then an aggressive one.
        (__c0, __c1) if (__c0 == V::Monster::FABRICATOR && __c1 == V::MoveId::FABRICATE_MOVE) => {
            spawn_bot::<V, _>(fight, V::GLORY_DEFENSE_SPAWNS);
            spawn_bot::<V, _>(fight, V::GLORY_AGGRO_SPAWNS);
        }
        // `FabricatingStrikeMove`: its hit, then an aggressive bot.
        (__c0, __c1)
            if (__c0 == V::Monster::FABRICATOR && __c1 == V::MoveId::FABRICATING_STRIKE_MOVE) =>
        {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            spawn_bot::<V, _>(fight, V::GLORY_AGGRO_SPAWNS);
        }
        // `GuardMove`: fifteen block, unpowered, on each Fabricator.
        (__c0, __c1) if (__c0 == V::Monster::GUARDBOT && __c1 == V::MoveId::GUARD_MOVE) => {
            use sts2_core::{Amount, CreatureView as _, Effect, Scaling};

            let fabricators: Vec<_> = fight
                .creatures()
                .filter(|creature| creature.monster() == Some(V::Monster::FABRICATOR))
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
        (__c0, __c1) if (__c0 == V::Monster::NOISEBOT && __c1 == V::MoveId::NOISE_MOVE) => {
            let dazed = V::Card::DAZED;
            fight.add_generated_cards(
                sts2_core::CardIdent::new(dazed),
                sts2_core::Pile::Discard,
                1,
            );
            fight.add_at_random(dazed, sts2_core::Pile::Draw);
        }
        // `FlamethrowerMove`: four Burns into the hand of the one player.
        (__c0, __c1)
            if (__c0 == V::Monster::MECHA_KNIGHT && __c1 == V::MoveId::FLAMETHROWER_MOVE) =>
        {
            fight.add_generated_cards(
                sts2_core::CardIdent::new(V::Card::BURN),
                sts2_core::Pile::Hand,
                4,
            );
        }
        // `WindupMove`: fifteen block and five Strength. `IsWoundUp` picks its
        // animations alone.
        (__c0, __c1) if (__c0 == V::Monster::MECHA_KNIGHT && __c1 == V::MoveId::WINDUP_MOVE) => {
            use sts2_core::{Amount, Effect, Scaling};

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(15),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(5));
        }
        (__c0, __c1)
            if (__c0 == V::Monster::MECHA_KNIGHT && __c1 == V::MoveId::HEAVY_CLEAVE_MOVE) =>
        {
            fight.attack_opponents()
        }
        // `HexMove`: two Hex on each target.
        (__c0, __c1) if (__c0 == V::Monster::SPECTRAL_KNIGHT && __c1 == V::MoveId::HEX) => {
            fight.apply_to_opponents(V::Power::HEX_POWER, PowerAmount::Counted(2));
        }
        // `MagicBombMove`: its hit. The rest picks where its effect lands.
        (__c0, __c1) if (__c0 == V::Monster::MAGI_KNIGHT && __c1 == V::MoveId::MAGIC_BOMB) => {
            fight.attack_opponents()
        }
        // `DampenMove`: each opponent's Dampen takes the knight as a caster,
        // and one that has none yet gets it, applied from the knight.
        (__c0, __c1) if (__c0 == V::Monster::MAGI_KNIGHT && __c1 == V::MoveId::DAMPEN_MOVE) => {
            use sts2_core::CreatureView as _;

            let actor = fight.actor();
            for target in fight.opponents() {
                let fresh = fight
                    .creatures()
                    .find(|creature| creature.id() == target)
                    .is_none_or(|creature| creature.power(V::Power::DAMPEN_POWER).is_none());
                if fresh {
                    fight.apply_power_from(
                        target,
                        V::Power::DAMPEN_POWER,
                        PowerAmount::Counted(1),
                        Some(actor),
                    );
                }
                fight.keep_creature(target, V::Power::DAMPEN_POWER, actor);
            }
        }
        // `JudicialFlightMove`: Soar on itself. `IsFlying` picks its sounds
        // alone.
        (__c0, __c1)
            if (__c0 == V::Monster::OWL_MAGISTRATE && __c1 == V::MoveId::JUDICIAL_FLIGHT) =>
        {
            fight.apply_to_actor(V::Power::SOAR_POWER, PowerAmount::Counted(1));
        }
        // `VerdictMove`: its hit, four Vulnerable on each target, then Soar
        // comes off.
        (__c0, __c1) if (__c0 == V::Monster::OWL_MAGISTRATE && __c1 == V::MoveId::VERDICT) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(V::Power::VULNERABLE_POWER, PowerAmount::Counted(4));
            let actor = fight.actor();
            fight.remove_power(actor, V::Power::SOAR_POWER);
        }
        // `LeechingHugMove`: three Weak on each target from nobody, then
        // three Strength onto itself.
        (__c0, __c1)
            if (__c0 == V::Monster::SLIMED_BERSERKER && __c1 == V::MoveId::LEECHING_HUG_MOVE) =>
        {
            for target in fight.opponents() {
                fight.apply_power_from(target, V::Power::WEAK_POWER, PowerAmount::Counted(3), None);
            }
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(3));
        }
        // `IncreasingIntensityMove`: every Wither the player holds takes a
        // fake upgrade, the next ones one more, and then the Withers and the
        // Strength.
        (__c0, __c1)
            if (__c0 == V::Monster::AEONGLASS && __c1 == V::MoveId::INCREASING_INTENSITY_MOVE) =>
        {
            let wither = V::Card::WITHER;
            fight.modify_cards(
                sts2_core::Writing::EveryCopyOf(wither),
                sts2_core::Change::Raise(V::CardVar::DAMAGE, WITHER_RAISE),
            );
            // The Withers it gives read the count this raises.
            let bonus = fight.model(|model| {
                let crate::behaviour::monsters::Model::Aeonglass {
                    strength,
                    wither_upgrades,
                } = model
                else {
                    unreachable!("an Aeonglass holding {model:?}")
                };
                *wither_upgrades += 1;
                Number::from(*strength)
            });
            let deadly = fight.ascension().has(AscensionLevel::DeadlyEnemies);
            let (withers, base) = if deadly { (2, 4) } else { (1, 3) };
            fight.give_cards(wither, withers);
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(base + bonus));
            fight.model(|model| {
                let crate::behaviour::monsters::Model::Aeonglass { strength, .. } = model else {
                    unreachable!("an Aeonglass holding {model:?}")
                };
                *strength += 1;
            });
        }
        // `BurnBrightForMeMove`: Strength on each of its teammates, then its
        // block.
        (__c0, __c1)
            if (__c0 == V::Monster::QUEEN && __c1 == V::MoveId::BURN_BRIGHT_FOR_ME_MOVE) =>
        {
            use sts2_core::{Amount, CreatureView as _, Effect, Scaling};

            let actor = fight.actor();
            let side = fight.side();
            let teammates: Vec<_> = fight
                .creatures()
                .filter(|creature| {
                    creature.side() == side && creature.alive() && creature.id() != actor
                })
                .map(|creature| creature.id())
                .collect();
            for teammate in teammates {
                fight.apply_power_from(
                    teammate,
                    V::Power::STRENGTH_POWER,
                    PowerAmount::Counted(1),
                    Some(actor),
                );
            }
            let block = fight.resolve(Effect::GainBlock {
                recipient: actor,
                amount: Amount::Number(20),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
        }
        // `SoulBeamMove`: its three hits, aimed past the first target's node.
        (__c0, __c1)
            if (__c0 == V::Monster::TORCH_HEAD_AMALGAM && __c1 == V::MoveId::BEAM_MOVE) =>
        {
            fight.attack_opponents()
        }
        // `MultiClawMove`: its hits, then one more for the next.
        (__c0, __c1)
            if (__c0 == V::Monster::TEST_SUBJECT && __c1 == V::MoveId::MULTI_CLAW_MOVE) =>
        {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.model(|model| {
                let crate::behaviour::monsters::Model::TestSubject { extra_claws, .. } = model
                else {
                    unreachable!("a Test Subject holding {model:?}")
                };
                *extra_claws = extra_claws
                    .checked_add(1)
                    .unwrap_or_else(|| unimplemented!("a Multi Claw past {extra_claws} before it"));
            });
        }
        // `BurningGrowlMove`: Burns into the discard pile, then Strength.
        (__c0, __c1)
            if (__c0 == V::Monster::TEST_SUBJECT && __c1 == V::MoveId::BURNING_GROWL_MOVE) =>
        {
            let deadly = fight.ascension().has(AscensionLevel::DeadlyEnemies);
            let (burns, strength) = if deadly { (5, 3) } else { (3, 2) };
            fight.give_cards(V::Card::BURN, burns);
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(strength));
        }
        // `RespawnMove`: `AdaptablePower.DoRevive`, then the next form's
        // health and what it gains. The second respawn is the last, and
        // takes Adaptable and Painful Stabs off.
        (__c0, __c1) if (__c0 == V::Monster::TEST_SUBJECT && __c1 == V::MoveId::RESPAWN_MOVE) => {
            let respawns = fight.model(|model| {
                let crate::behaviour::monsters::Model::TestSubject { respawns, .. } = model else {
                    unreachable!("a Test Subject holding {model:?}")
                };
                *respawns += 1;
                *respawns
            });
            let actor = fight.actor();
            fight.write_power(actor, V::Power::ADAPTABLE_POWER, |state| {
                state.set_reviving(false)
            });
            let tough = fight.ascension().has(AscensionLevel::ToughEnemies);
            match respawns {
                1 => {
                    revive_at::<V, _>(fight, if tough { 212 } else { 200 });
                    fight.apply_to_actor(V::Power::PAINFUL_STABS_POWER, PowerAmount::Counted(1));
                }
                2 => {
                    revive_at::<V, _>(fight, if tough { 313 } else { 300 });
                    fight.apply_to_actor(V::Power::NEMESIS_POWER, PowerAmount::Counted(1));
                    fight.remove_power(actor, V::Power::ADAPTABLE_POWER);
                    fight.remove_power(actor, V::Power::PAINFUL_STABS_POWER);
                }
                _ => {}
            }
        }
        _ => return V::generated_move(monster, move_id, step, fight),
    }
    None
}

/// A Glory monster's attack damage that it works out from the fight.
pub(super) fn computed_damage<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    monster: V::Monster,
    move_id: V::MoveId,
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
        (__c0, __c1) if (__c0 == V::Monster::THE_FORGOTTEN && __c1 == V::MoveId::DREAD) => {
            (if deadly { 15 } else { 13 }) + held(V::Power::DEXTERITY_POWER)
        }
        (monster, move_id) => unimplemented!("the damage {monster:?} works out for {move_id:?}"),
    }
}

/// `PunchConstruct.AfterAddedToRoom`: `Math.Max(1, CurrentHp -
/// StartingHpReduction)` where the roster wrote a reduction.
pub(super) fn hp_after_added<V: crate::Build>(
    monster: V::Monster,
    model: &crate::behaviour::monsters::Model<V>,
    hp: i32,
) -> Option<i32> {
    match (monster, *model) {
        (__c0, crate::behaviour::monsters::Model::PunchConstruct { hp_reduction })
            if __c0 == V::Monster::PUNCH_CONSTRUCT && (hp_reduction > 0) =>
        {
            Some((hp - i32::from(hp_reduction)).max(1))
        }
        _ => None,
    }
}

/// `DEFENSE_SPAWNS`, `AGGRO_SPAWNS`, written for one build's own types.
macro_rules! lists {
    ($build:ty, v0_107_1) => {
        crate::behaviour::monsters::glory::lists!($build);
        /// Globe Head's Galvanic from Deadly Enemies on, which this build
        /// leaves at its base.
        const GLOBE_HEAD_DEADLY_GALVANIC: crate::behaviour::monsters::Number = 6;
    };
    ($build:ty, v0_111_0) => {
        crate::behaviour::monsters::glory::lists!($build);
        /// Globe Head's `GalvanicPowerAmount` from Deadly Enemies on.
        const GLOBE_HEAD_DEADLY_GALVANIC: crate::behaviour::monsters::Number = 8;
    };
    ($build:ty) => {
        /// `Fabricator.defenseSpawns`, in the order the set lists them.
        const GLORY_DEFENSE_SPAWNS: &'static [<$build as sts2_core::ContentVersion>::Monster] = &[
            <$build as sts2_core::ContentVersion>::Monster::GUARDBOT,
            <$build as sts2_core::ContentVersion>::Monster::NOISEBOT,
        ];
        /// `Fabricator.aggroSpawns`.
        const GLORY_AGGRO_SPAWNS: &'static [<$build as sts2_core::ContentVersion>::Monster] = &[
            <$build as sts2_core::ContentVersion>::Monster::ZAPBOT,
            <$build as sts2_core::ContentVersion>::Monster::STABBOT,
        ];
    };
}

pub(crate) use lists;
