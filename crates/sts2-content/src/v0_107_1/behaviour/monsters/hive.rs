//! The second act's monsters.

use sts2_core::{
    Amount, CardAt, CardTable as _, Effect, Fight, Pile, PowerAmount, RunStreamKinds as _, Scaling,
};

use super::super::super::generated::ascension::AscensionLevel;
use super::super::super::generated::cards::CardRarity;
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::super::super::generated::powers::Power;
use super::{Number, V};

/// What a creature's health is counted in.
type Health = <V as sts2_core::ContentVersion>::Health;

/// Powers a Hive monster applies to itself as it enters combat.
pub(super) fn starting_powers(
    monster: Monster,
    ascension: AscensionLevel,
) -> Vec<(Power, PowerAmount<Number>)> {
    // `PlatingPower`'s own amount, which is also the block it opens with.
    const PLATING: Number = 15;
    const TOUGH_PLATING: Number = 18;

    // `SlumberPower`, which counts the hits the beetle sleeps through.
    const SLUMBER: Number = 3;

    // `ReattachPower`, which is both what keeps a segment in the fight and
    // the health its reattach gives back.
    const REATTACH: Number = 25;

    match monster {
        Monster::ThievingHopper => vec![(Power::EscapeArtistPower, PowerAmount::Counted(5))],
        Monster::DecimillipedeSegmentFront
        | Monster::DecimillipedeSegmentMiddle
        | Monster::DecimillipedeSegmentBack => {
            vec![(Power::ReattachPower, PowerAmount::Counted(REATTACH))]
        }
        Monster::BowlbugRock => vec![(Power::ImbalancedPower, PowerAmount::Counted(1))],
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
        _ => Vec::new(),
    }
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
        monster => unimplemented!("what {monster:?} asks the fight at state {at}"),
    };
    super::state_of(monster, wanted, at)
}

/// Perform one of the Hive monsters' moves.
pub(super) fn perform<F: Fight<V>>(move_id: MoveId, fight: &mut F) {
    match move_id {
        MoveId::ThieveryMove => thievery(fight),
        MoveId::BiteMove
        | MoveId::BelowMove
        | MoveId::PunctureMove
        | MoveId::HeadbuttMove
        | MoveId::ThrashMove => fight.attack_opponents(),
        MoveId::BurrowMove => burrow(fight),
        MoveId::TenderizingGoopMove => tenderizing_goop(fight),
        MoveId::ToxicSpitMove => {
            fight.apply_to_opponents(Power::WeakPower, PowerAmount::Counted(1))
        }
        // `SnoreMove` sleeps, which is a sound and nothing else.
        MoveId::SnoreMove => {}
        MoveId::RollOutMove => roll_out(fight),
        // `WritheMove`, whose two hits are the intent's own repeat count.
        MoveId::WritheMove => fight.attack_opponents(),
        MoveId::BulkMove => bulk(fight),
        MoveId::ConstrictMove => constrict(fight),
        // `DeadMove` is a segment lying where it fell, which is nothing.
        MoveId::DeadMove => {}
        MoveId::ReattachMove => reattach(fight),
        move_id => unimplemented!("what {move_id:?} does when it is performed"),
    }
}

/// Make the target tender: every card it plays costs it a point of each.
///
/// One stack, however many targets the move names. The power itself is what
/// takes the Strength and the Dexterity, and what hands them back.
fn tenderizing_goop<F: Fight<V>>(fight: &mut F) {
    fight.apply_to_opponents(Power::TenderPower, PowerAmount::Counted(1));
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

/// Hide underground behind block that persists until it is broken.
fn burrow<F: Fight<V>>(fight: &mut F) {
    const BLOCK: Number = 32;
    const TOUGH_BLOCK: Number = 37;

    fight.apply_to_actor(Power::BurrowedPower, PowerAmount::Counted(1));
    let amount = match fight.ascension().has(AscensionLevel::ToughEnemies) {
        true => TOUGH_BLOCK,
        false => BLOCK,
    };
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Number(amount),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
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
                    let imbued = ident.enchantment == Some(Enchantment::Imbued);
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
