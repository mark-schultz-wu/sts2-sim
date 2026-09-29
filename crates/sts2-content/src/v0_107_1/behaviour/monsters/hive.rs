//! The second act's monsters.

use sts2_core::{
    Amount, CardAt, CardTable as _, Effect, Fight, Pile, PowerAmount, RunStreamKinds as _, Scaling,
};

use super::super::super::generated::ascension::AscensionLevel;
use super::super::super::generated::cards::{CardRarity, Cards, StatusCard};
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::monster_moves;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::super::super::generated::powers::Power;
use super::super::super::generated::runstate::CombatSide;
use super::{Number, Resume, V};

/// `FlutterMove`'s `PowerCmd.Apply<FlutterPower>(…, 5m, …)`.
const FLUTTER: Number = 5;

/// What a creature's health is counted in.
type Health = <V as sts2_core::ContentVersion>::Health;

/// Powers a Hive monster applies to itself as it enters combat.
pub(super) fn starting_powers(
    monster: Monster,
    ascension: AscensionLevel,
    side: CombatSide,
) -> Option<Vec<(Power, PowerAmount<Number>)>> {
    // `PlatingPower`'s own amount, which is also the block it opens with.
    const PLATING: Number = 15;
    const TOUGH_PLATING: Number = 18;

    // `SlumberPower`, which counts the hits the beetle sleeps through.
    const SLUMBER: Number = 3;

    // `ReattachPower`, which is both what keeps a segment in the fight and
    // the health its reattach gives back.
    const REATTACH: Number = 25;

    Some(match monster {
        Monster::ThievingHopper => vec![(Power::EscapeArtistPower, PowerAmount::Counted(5))],
        Monster::DecimillipedeSegmentFront
        | Monster::DecimillipedeSegmentMiddle
        | Monster::DecimillipedeSegmentBack => {
            vec![(Power::ReattachPower, PowerAmount::Counted(REATTACH))]
        }
        Monster::BowlbugRock => vec![(Power::ImbalancedPower, PowerAmount::Counted(1))],
        Monster::Exoskeleton => vec![(Power::HardToKillPower, PowerAmount::Counted(9))],
        // Each of these calls the base alone. The Ovicopter starts a sound
        // loop beside it.
        Monster::BowlbugEgg | Monster::SpinyToad | Monster::Ovicopter => Vec::new(),
        Monster::Chomper => vec![(Power::ArtifactPower, PowerAmount::Counted(2))],
        Monster::Entomancer => vec![(Power::PersonalHivePower, PowerAmount::Counted(1))],
        // The Rocket's `AfterAddedToRoom` also surrounds its opponents, which
        // `starting_opponent_powers` names.
        Monster::Crusher => vec![
            (Power::BackAttackLeftPower, PowerAmount::Counted(1)),
            (Power::CrabRagePower, PowerAmount::Counted(1)),
        ],
        Monster::Rocket => vec![
            (Power::BackAttackRightPower, PowerAmount::Counted(1)),
            (Power::CrabRagePower, PowerAmount::Counted(1)),
        ],
        Monster::InfestedPrism => {
            let spark = match ascension.has(AscensionLevel::DeadlyEnemies) {
                true => 3,
                false => 2,
            };
            vec![(Power::VitalSparkPower, PowerAmount::Counted(spark))]
        }
        Monster::Parafright => vec![(Power::IllusionPower, PowerAmount::Counted(1))],
        // `ToughEgg.AfterAddedToRoom`, for an egg not yet hatched: Hatch at
        // two while the enemies take their turn, and at one otherwise.
        Monster::ToughEgg => {
            let hatch = match side {
                CombatSide::Enemy => 2,
                _ => 1,
            };
            vec![(Power::HatchPower, PowerAmount::Counted(hatch))]
        }
        Monster::LouseProgenitor => {
            vec![(
                Power::CurlUpPower,
                PowerAmount::Counted(curl_block(ascension)),
            )]
        }
        Monster::SlumberingBeetle => {
            let plating = match ascension.has(AscensionLevel::ToughEnemies) {
                true => TOUGH_PLATING,
                false => PLATING,
            };
            vec![
                (Power::PlatingPower, PowerAmount::Counted(plating)),
                (Power::SlumberPower, PowerAmount::Counted(SLUMBER)),
            ]
        }
        _ => return None,
    })
}

/// What a Hive monster's `AfterAddedToRoom` writes over its rolled health.
///
/// `DecimillipedeSegment` is the one that does. A segment carries an even
/// number of points, and no two in the row carry the same number. The player
/// can then tell them apart by the bar alone. It walks up in twos from the
/// rolled number, and wraps round its own range rather than leave it.
pub(super) fn health_after_added(
    monster: Monster,
    ascension: AscensionLevel,
    rolled: Health,
    teammates: &[Health],
) -> Option<Health> {
    if !matches!(
        monster,
        Monster::DecimillipedeSegmentFront
            | Monster::DecimillipedeSegmentMiddle
            | Monster::DecimillipedeSegmentBack
    ) {
        return None;
    }
    // `MaxInitialHp` and `MinInitialHp`, which are the same two numbers the
    // health was rolled between.
    let data = sts2_core::Table::data(monster);
    let (lowest, highest): (Health, Health) = (
        data.min_hp.at(ascension as u8).into(),
        data.max_hp.at(ascension as u8).into(),
    );
    let mut health = rolled;
    if health % 2 == 1 {
        health += 1;
    }
    while teammates.contains(&health) {
        health += 2;
        if health > highest {
            health = lowest;
        }
    }
    Some(health)
}

/// Which move a Hive monster selects from a conditional state.
pub(super) fn conditional<P: sts2_core::Printed<V> + ?Sized>(
    monster: Monster,
    at: u8,
    spawn: sts2_core::MonsterSpawn<super::super::super::generated::encounters::Encounter>,
    powers: &[sts2_core::Listener<'_, V, P>],
) -> Result<u8, sts2_core::ContentError> {
    let held = |wanted| {
        powers.iter().find_map(|listener| match listener {
            sts2_core::Listener::Power { power, state, .. } if *power == wanted => Some(*state),
            _ => None,
        })
    };
    let wanted = match monster {
        Monster::BowlbugRock => match held(Power::ImbalancedPower) {
            Some(state) if state.is_off_balance() => MoveId::DizzyMove,
            _ => MoveId::HeadbuttMove,
        },
        Monster::SlumberingBeetle => match held(Power::SlumberPower) {
            Some(_) => MoveId::SnoreMove,
            None => MoveId::RollOutMove,
        },
        // `CanLay`: three or fewer stand on its side, itself included.
        Monster::Ovicopter => match spawn.standing <= LAYS_WHILE {
            true => MoveId::LayEggsMove,
            false => MoveId::NutritionalPasteMove,
        },
        monster => unimplemented!("what {monster:?} asks the fight at state {at}"),
    };
    super::state_of(monster, wanted, at)
}

/// Perform one of the Hive monsters' moves, where the generator left it to a
/// hand port.
pub(super) fn perform<F: Fight<V>>(
    monster: Monster,
    move_id: MoveId,
    step: u8,
    fight: &mut F,
) -> Option<u8> {
    let segment = matches!(
        monster,
        Monster::DecimillipedeSegmentFront
            | Monster::DecimillipedeSegmentMiddle
            | Monster::DecimillipedeSegmentBack
    );
    match (monster, move_id) {
        (Monster::ThievingHopper, MoveId::ThieveryMove) => thievery(fight),
        // `WebMove`: its hit, then Frail at two on each target. `Curled`
        // picks its animations alone.
        (Monster::LouseProgenitor, MoveId::WebCannonMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(Power::FrailPower, PowerAmount::Counted(2));
        }
        (Monster::LouseProgenitor, MoveId::PounceMove) => fight.attack_opponents(),
        // `ToxicMove`: `CardPileCmd.AddToCombatAndPreview<Toxic>(targets,
        // Hand, 2)`, into the hand of the one player.
        // `LayEggsMove`: three times, `CreatureCmd.Add<ToughEgg>` into the
        // last slot no enemy holds, then Minion on it.
        (Monster::Ovicopter, MoveId::LayEggsMove) => {
            for _ in 0..EGGS {
                if let Some(egg) = fight.summon_into_last_free_slot(Monster::ToughEgg) {
                    fight.apply_power(egg, Power::MinionPower, PowerAmount::Counted(1));
                }
            }
        }
        // `HatchMove`: Hatch goes, then every other power but Minion, then
        // `Hatch`: its hit points set to a draw off the niche stream.
        (Monster::ToughEgg, MoveId::HatchMove) => {
            use sts2_core::{CreatureView as _, Table as _};

            let egg = fight.actor();
            fight.remove_power(egg, Power::HatchPower);
            let others: Vec<Power> = fight
                .creatures()
                .find(|creature| creature.id() == egg)
                .map(|creature| {
                    Power::ALL
                        .iter()
                        .copied()
                        .filter(|&power| power != Power::MinionPower)
                        .filter(|&power| creature.power(power).is_some())
                        .collect()
                })
                .unwrap_or_default();
            for power in others {
                fight.remove_power(egg, power);
            }
            let (min, max) = match fight.ascension().has(AscensionLevel::ToughEnemies) {
                true => (20, 23),
                false => (19, 22),
            };
            let hp = fight.niche_int(min, max);
            fight.set_max_and_current_hp(egg, num_traits::cast(hp).expect("a hatchling's health"));
        }
        // `IllusionMove`: `CreatureCmd.Add<Parafright>` into the `illusion`
        // slot while the fight is live.
        (Monster::TheObscura, MoveId::IllusionMove) => {
            fight.summon_into_slot(Monster::Parafright, ILLUSION_SLOT);
        }
        // `WailMove`: Strength at three on every creature on its side.
        (Monster::TheObscura, MoveId::SailMove) => {
            use sts2_core::CreatureView as _;

            let side = fight.side();
            let team: Vec<_> = fight
                .creatures()
                .filter(|creature| creature.side() == side && creature.alive())
                .map(|creature| creature.id())
                .collect();
            for creature in team {
                fight.apply_power(creature, Power::StrengthPower, PowerAmount::Counted(3));
            }
        }
        // `SpitMove`: below three Personal Hive, one more of it and one
        // Strength. At three, two Strength.
        (Monster::Entomancer, MoveId::PheromoneSpitMove) => {
            use sts2_core::CreatureView as _;

            let actor = fight.actor();
            let hive = fight
                .creatures()
                .find(|creature| creature.id() == actor)
                .and_then(|creature| creature.power(Power::PersonalHivePower));
            match hive {
                Some(PowerAmount::Counted(amount)) if amount < HIVE_CAP => {
                    fight.apply_to_actor(Power::PersonalHivePower, PowerAmount::Counted(1));
                    fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(1));
                }
                Some(PowerAmount::Counted(_)) => {
                    fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(2))
                }
                Some(PowerAmount::Endless) => unimplemented!("an endless Personal Hive"),
                None => unimplemented!("an Entomancer spitting with no Personal Hive"),
            }
        }
        // The Kaiser Crab's arms. The rest of each move is its background's
        // animation.
        (Monster::Crusher, MoveId::ThrashMove | MoveId::EnlargingStrikeMove) => {
            fight.attack_opponents()
        }
        // `BugStingMove`: its two hits, then Weak and Frail at two on each
        // target.
        (Monster::Crusher, MoveId::BugStingMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(Power::WeakPower, PowerAmount::Counted(2));
            fight.apply_to_opponents(Power::FrailPower, PowerAmount::Counted(2));
        }
        (Monster::Crusher, MoveId::AdaptMove) => {
            let strength = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => 3,
                false => 2,
            };
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(strength));
        }
        // `GuardedStrikeMove`: its hit, then eighteen block as a move.
        (Monster::Crusher, MoveId::GuardedStrikeMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(GUARDED_BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
        }
        (
            Monster::Rocket,
            MoveId::TargetingReticleMove | MoveId::PrecisionBeamMove | MoveId::LaserMove,
        ) => fight.attack_opponents(),
        (Monster::Rocket, MoveId::ChargeUpMove) => {
            let strength = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => 3,
                false => 2,
            };
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(strength));
        }
        (Monster::Rocket, MoveId::RechargeMove) => {}
        // `LiquifyMove`: Sandpit at four on itself, aimed at the player, then
        // six Frantic Escapes at random places: three into the draw pile and
        // three into the discard pile. `HasLiquified` picks its animations
        // alone. A fight here has one player.
        (Monster::TheInsatiable, MoveId::LiquifyGroundMove) => {
            fight.apply_to_actor(Power::SandpitPower, PowerAmount::Counted(SANDPIT));
            for escape in 0..ESCAPES {
                let pile = match escape < ESCAPES / 2 {
                    true => Pile::Draw,
                    false => Pile::Discard,
                };
                fight.add_at_random(Cards::Status(StatusCard::FranticEscape), pile);
            }
        }
        (Monster::Myte, MoveId::ToxicMove) => {
            fight.add_generated_cards(
                sts2_core::CardIdent::new(Cards::Status(StatusCard::Toxic)),
                Pile::Hand,
                2,
            );
        }
        // `SpikesMove`: Thorns at five on itself. `IsSpiny` picks its
        // animations alone.
        (Monster::SpinyToad, MoveId::ProtrudingSpikesMove) => {
            fight.apply_to_actor(Power::ThornsPower, PowerAmount::Counted(SPINES))
        }
        // `ExplosionMove`: its hit, then the same Thorns taken back.
        (Monster::SpinyToad, MoveId::SpikeExplosionMove) => {
            if super::attack_first(fight, step) {
                return Some(1);
            }
            fight.apply_to_actor(Power::ThornsPower, PowerAmount::Counted(-SPINES));
        }
        // `CurlAndGrowMove`: `CreatureCmd.GainBlock` of its `CurlBlock` as a
        // move, then Strength at five on itself.
        (Monster::LouseProgenitor, MoveId::CurlAndGrowMove) => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(curl_block(fight.ascension())),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(5));
        }
        // `FlutterMove`: `IsHovering`, which picks its sounds, then
        // `PowerCmd.Apply<FlutterPower>` of five on itself.
        (Monster::ThievingHopper, MoveId::FlutterMove) => {
            fight.apply_to_actor(Power::FlutterPower, PowerAmount::Counted(FLUTTER))
        }
        // `EscapeMove`: `CreatureCmd.Escape` on itself.
        (Monster::ThievingHopper, MoveId::EscapeMove) => fight.escape(),
        (Monster::BowlbugRock, MoveId::HeadbuttMove) => fight.attack_opponents(),
        (Monster::SlumberingBeetle, MoveId::RollOutMove) => return roll_out(fight, step),
        // `WritheMove`, whose two hits are the intent's own repeat count.
        (_, MoveId::WritheMove) if segment => fight.attack_opponents(),
        (_, MoveId::BulkMove) if segment => return bulk(fight, step),
        (_, MoveId::ConstrictMove) if segment => return constrict(fight, step),
        (_, MoveId::ReattachMove) if segment => reattach(fight),
        (monster, move_id) => return monster_moves::hive::perform(monster, move_id, step, fight),
    }
    None
}

/// Roll over the target, then harden.
///
/// `RolloutMove` applies the Strength to the beetle itself, so the fight it
/// hits and the creature it grows are not the same one.
fn roll_out<F: Fight<V>>(fight: &mut F, step: u8) -> Option<u8> {
    const HARDENS: Number = 2;

    if super::attack_first(fight, step) {
        return Some(1);
    }
    fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(HARDENS));
    None
}

/// Swell: hit, then grow.
///
/// `BulkMove` applies the Strength to the segment itself, so the creature it
/// hits and the one it grows are not the same.
fn bulk<F: Fight<V>>(fight: &mut F, step: u8) -> Option<u8> {
    const BULKS: Number = 2;

    if super::attack_first(fight, step) {
        return Some(1);
    }
    fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(BULKS));
    None
}

/// Squeeze: hit, and leave the target weakened.
fn constrict<F: Fight<V>>(fight: &mut F, step: u8) -> Option<u8> {
    if super::attack_first(fight, step) {
        return Some(1);
    }
    fight.apply_to_opponents(Power::WeakPower, PowerAmount::Counted(1));
    None
}

/// Join the row again, for the health the power carries.
///
/// `ReattachPower.DoReattach`, which is `CreatureCmd.Heal(Owner, Amount)` and
/// so is what brings the segment back. A row whose every other segment is down
/// does not reattach. The fight is over by then, and what plays instead is the
/// death of all of them at once.
fn reattach<F: Fight<V>>(fight: &mut F) {
    use sts2_core::CreatureView as _;

    // `GetOtherSegments`, which is the rest of the row carrying this power.
    let actor = fight.actor();
    let side = fight.side();
    let alone = fight
        .creatures()
        .filter(|creature| {
            creature.id() != actor
                && creature.side() == side
                && creature.power(Power::ReattachPower).is_some()
        })
        .all(|creature| !creature.alive());
    if alone {
        return;
    }
    let carried = fight
        .creatures()
        .find(|creature| creature.id() == actor)
        .and_then(|creature| creature.power(Power::ReattachPower));
    let amount = match carried {
        Some(PowerAmount::Counted(amount)) => amount,
        Some(PowerAmount::Endless) => unimplemented!("an endless Reattach power"),
        None => unreachable!("a segment reattaching carries the power that reattaches it"),
    };
    fight.write_power(actor, Power::ReattachPower, |state| {
        state.set_reviving(false)
    });
    fight.heal(actor, amount);
}

/// `LiquifyMove`'s Sandpit, `PowerCmd.Apply(…, 4m, …)`.
const SANDPIT: Number = 4;

/// `LiquifyMove`'s six Frantic Escapes.
const ESCAPES: usize = 6;

/// `GuardedStrikeMove`'s `CreatureCmd.GainBlock(…, 18m, ValueProp.Move, …)`.
const GUARDED_BLOCK: Number = 18;

/// The Personal Hive `SpitMove` stops growing at.
const HIVE_CAP: Number = 3;

/// `illusion`, the first of `TheObscuraNormal.Slots`.
const ILLUSION_SLOT: u8 = 0;

/// `LayEggsMove`'s loop, which lays three.
const EGGS: usize = 3;

/// `CanLay`'s bound on how many stand on the Ovicopter's side.
const LAYS_WHILE: u8 = 3;

/// The Thorns `SpikesMove` applies and `ExplosionMove` takes back.
const SPINES: Number = 5;

/// `LouseProgenitor.CurlBlock`, either side of `ToughEnemies`.
fn curl_block(ascension: <V as sts2_core::ContentVersion>::AscensionLevel) -> Number {
    match ascension.has(AscensionLevel::ToughEnemies) {
        true => 18,
        false => 14,
    }
}

/// Take the highest-priority deck card still in combat, then attack.
fn thievery<F: Fight<V>>(fight: &mut F) {
    let cards: Vec<CardAt<V>> = [Pile::Draw, Pile::Discard]
        .into_iter()
        .flat_map(|pile| fight.cards_in(pile))
        .filter(|card| fight.deck_version(card.id()).is_some())
        .collect();
    let priorities = [
        &[CardRarity::Uncommon][..],
        &[CardRarity::Common, CardRarity::Rare, CardRarity::Event][..],
        &[CardRarity::Basic, CardRarity::Quest][..],
        &[CardRarity::Ancient][..],
    ];
    let candidates = priorities
        .into_iter()
        .find_map(|rarities| {
            let matching: Vec<_> = cards
                .iter()
                .copied()
                .filter(|&card| {
                    let ident = fight.card_ident(card);
                    let imbued = ident.enchantment.map(|enchanted| enchanted.enchantment)
                        == Some(Enchantment::Imbued);
                    let priority_accepts_imbued = rarities == [CardRarity::Ancient];
                    (imbued == priority_accepts_imbued)
                        && (rarities.contains(&ident.card.rarity()) || imbued)
                })
                .collect();
            (!matching.is_empty()).then_some(matching)
        })
        .unwrap_or(cards);
    if let Some(card) = fight.random_card(
        &candidates,
        <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
    ) {
        // `SwipePower.Steal`, then `PowerCmd.Apply`. A Swipe the fight refuses
        // takes the card with nobody to give it back.
        let stolen = fight
            .deck_version(card.id())
            .unwrap_or_else(|| unreachable!("the candidates are deck copies"));
        fight.remove_card(card);
        fight.apply_to_actor(Power::SwipePower, PowerAmount::Counted(1));
        let actor = fight.actor();
        fight.write_power(actor, Power::SwipePower, |state| state.steal(stolen));
    }
    fight.attack_opponents();
}

/// `CurseOfKnowledgeBranch`: a curse while any set is left, a slap once the
/// three are cast.
pub(super) fn knowledge_demon_branch(
    model: &super::Model,
    at: u8,
) -> Result<u8, sts2_core::ContentError> {
    let super::Model::KnowledgeDemon { curses } = *model else {
        unimplemented!("a Knowledge Demon holding {model:?}")
    };
    let wanted = match usize::from(curses) < CURSE_SETS {
        true => MoveId::CurseOfKnowledgeMove,
        false => MoveId::SlapMove,
    };
    super::state_of(Monster::KnowledgeDemon, wanted, at)
}

/// `_curseOfKnowledgeSets.Count`.
const CURSE_SETS: usize = 3;

/// The Knowledge Demon's moves.
///
/// `CurseOfKnowledge` offers the player `Disintegration` and one more card of
/// the set its counter stands at. The card chosen applies its power, which
/// `resume` carries out.
pub(super) fn knowledge_demon<F: Fight<V>>(
    move_id: MoveId,
    step: u8,
    fight: &mut F,
) -> sts2_core::Played<V> {
    use sts2_core::CardIdent;

    use super::super::super::generated::cards::TokenCard;

    // `ThrowingPlayerChoiceContext`'s `PowerCmd.Apply<StrengthPower>`.
    const PONDER_STRENGTH: Number = 2;
    const DEADLY_PONDER_STRENGTH: Number = 3;
    // `30 * Players.Count`, and a fight here has one player.
    const PONDER_HEAL: Number = 30;

    let deadly = fight.ascension().has(AscensionLevel::DeadlyEnemies);
    match move_id {
        MoveId::CurseOfKnowledgeMove => {
            // `CurseOfKnowledgeCounter++`, after the choice in the game.
            // Nothing reads it before the enemy turn is over.
            let set = fight.model(|model| {
                let super::Model::KnowledgeDemon { curses } = model else {
                    unimplemented!("a Knowledge Demon holding {model:?}")
                };
                let set = *curses;
                *curses = set + 1;
                set
            });
            let other = match set {
                0 => TokenCard::MindRot,
                1 => TokenCard::Sloth,
                2 => TokenCard::WasteAway,
                _ => unimplemented!("a Knowledge Demon with no set left at {set}"),
            };
            sts2_core::Played::Choosing {
                offered: [
                    CardIdent::new(Cards::Token(TokenCard::Disintegration)),
                    CardIdent::new(Cards::Token(other)),
                ]
                .into_iter()
                .collect(),
                skippable: true,
                at: super::super::Resume::Monster(Resume::CurseOfKnowledge { set }),
            }
        }
        // `IsBurnt` picks the demon's animations alone.
        MoveId::KnowledgeOverwhelmingMove | MoveId::SlapMove => {
            fight.attack_opponents();
            sts2_core::Played::Done
        }
        MoveId::PonderMove => {
            if super::attack_first(fight, step) {
                return sts2_core::Played::Waiting {
                    at: super::super::Resume::Monster(Resume::Step(1)),
                };
            }
            let actor = fight.actor();
            fight.heal(actor, PONDER_HEAL);
            let strength = match deadly {
                true => DEADLY_PONDER_STRENGTH,
                false => PONDER_STRENGTH,
            };
            fight.apply_power(actor, Power::StrengthPower, PowerAmount::Counted(strength));
            sts2_core::Played::Done
        }
        move_id => unimplemented!("what {move_id:?} does when a Knowledge Demon performs it"),
    }
}

/// `IChoosable.OnChosen` for the card the player took, which applies its
/// power to them. A skipped choice does nothing.
pub(super) fn resume<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: Option<sts2_core::CardIdent<V>>,
) -> sts2_core::Played<V> {
    use super::super::super::generated::cards::TokenCard;

    // `_disintegrationDamageValues`.
    const DISINTEGRATION: [Number; CURSE_SETS] = [6, 7, 8];

    let Resume::CurseOfKnowledge { set } = at else {
        unreachable!("{at:?} resumes a move, not a choice")
    };
    let Some(chosen) = chosen else {
        return sts2_core::Played::Done;
    };
    let (power, amount) = match chosen.card {
        Cards::Token(TokenCard::Disintegration) => {
            (Power::DisintegrationPower, DISINTEGRATION[usize::from(set)])
        }
        Cards::Token(TokenCard::MindRot) => (Power::MindRotPower, 1),
        Cards::Token(TokenCard::Sloth) => (Power::SlothPower, 3),
        Cards::Token(TokenCard::WasteAway) => (Power::WasteAwayPower, 1),
        card => unimplemented!("{card:?} chosen from a curse of knowledge"),
    };
    let actor = fight.actor();
    fight.apply_power(actor, power, PowerAmount::Counted(amount));
    sts2_core::Played::Done
}
