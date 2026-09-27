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
use super::{Number, V};

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
pub(super) fn perform<F: Fight<V>>(monster: Monster, move_id: MoveId, fight: &mut F) {
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
            fight.attack_opponents();
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
            fight.attack_opponents();
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
        (Monster::SlumberingBeetle, MoveId::RollOutMove) => roll_out(fight),
        // `WritheMove`, whose two hits are the intent's own repeat count.
        (_, MoveId::WritheMove) if segment => fight.attack_opponents(),
        (_, MoveId::BulkMove) if segment => bulk(fight),
        (_, MoveId::ConstrictMove) if segment => constrict(fight),
        (_, MoveId::ReattachMove) if segment => reattach(fight),
        (monster, move_id) => monster_moves::perform(monster, move_id, fight),
    }
}

/// Roll over the target, then harden.
///
/// `RolloutMove` applies the Strength to the beetle itself, so the fight it
/// hits and the creature it grows are not the same one.
fn roll_out<F: Fight<V>>(fight: &mut F) {
    const HARDENS: Number = 2;

    fight.attack_opponents();
    fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(HARDENS));
}

/// Swell: hit, then grow.
///
/// `BulkMove` applies the Strength to the segment itself, so the creature it
/// hits and the one it grows are not the same.
fn bulk<F: Fight<V>>(fight: &mut F) {
    const BULKS: Number = 2;

    fight.attack_opponents();
    fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(BULKS));
}

/// Squeeze: hit, and leave the target weakened.
fn constrict<F: Fight<V>>(fight: &mut F) {
    fight.attack_opponents();
    fight.apply_to_opponents(Power::WeakPower, PowerAmount::Counted(1));
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
        let stolen = fight.remove_card(card);
        fight.apply_to_actor(Power::SwipePower, PowerAmount::Counted(1));
        let actor = fight.actor();
        fight.write_power(actor, Power::SwipePower, |state| state.steal(stolen));
    }
    fight.attack_opponents();
}
