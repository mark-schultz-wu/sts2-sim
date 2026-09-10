//! Card rewards drawn from a player's reward stream.

use sts2_core::rng::{Bound, GameRng};
use sts2_core::{Drafted, RewardRarity, Rules};

use super::Odds;

/// Draw distinct cards with regular-encounter rarity odds.
///
/// Each card costs three draws: rarity, the item in that rarity, and the
/// upgrade roll. At Neow the upgrade chance is zero, but the roll still happens.
pub(crate) fn regular<V: Rules, R: GameRng>(
    rng: &mut R,
    cards: Drafted<'_, V::Card>,
    count: usize,
) -> Vec<V::Card> {
    let mut drawn = Vec::with_capacity(count);
    for _ in 0..count {
        let roll = rng.next_double() as f32;
        let wanted = if roll < 0.03 {
            RewardRarity::Rare
        } else if roll < 0.37 {
            RewardRarity::Uncommon
        } else {
            RewardRarity::Common
        };
        let choices = available::<V>(cards, &drawn, wanted);
        let left = choices.iter().filter(|card| !drawn.contains(card)).count() as i32;
        let bound = Bound::new(left).expect("an available rarity has a card");
        let at = rng.next_int(bound) as usize;
        drawn.push(
            choices
                .iter()
                .filter(|card| !drawn.contains(card))
                .nth(at)
                .copied()
                .expect("the draw is inside the count"),
        );
        let _upgrade_roll = rng.next_double();
    }
    drawn
}

fn available<'a, V: Rules>(
    cards: Drafted<'a, V::Card>,
    drawn: &[V::Card],
    wanted: RewardRarity,
) -> &'a [V::Card] {
    let order = match wanted {
        RewardRarity::Common => [
            RewardRarity::Common,
            RewardRarity::Uncommon,
            RewardRarity::Rare,
        ],
        RewardRarity::Uncommon => [
            RewardRarity::Uncommon,
            RewardRarity::Rare,
            RewardRarity::Common,
        ],
        RewardRarity::Rare => [
            RewardRarity::Rare,
            RewardRarity::Common,
            RewardRarity::Uncommon,
        ],
        RewardRarity::Other => unreachable!("a reward roll never asks for another rarity"),
    };
    order
        .into_iter()
        .map(|rarity| cards.at(rarity))
        .find(|cards| cards.iter().any(|card| !drawn.contains(card)))
        .expect("a card pool has a drafted rarity")
}

/// The odds an encounter's own card reward rolls a rare at, before the run's
/// running offset.
const RARE: f32 = 0.03;

/// The odds of an uncommon, which the game adds to the rare threshold rather
/// than reading on its own.
const UNCOMMON: f32 = 0.37;

/// Where the offset goes back to once a rare lands.
const OFFSET_BASE: f32 = -0.05;

/// What a roll that is not a rare adds to the offset, and the cap it climbs to.
const OFFSET_GROWTH: f32 = 0.01;
const OFFSET_MAX: f32 = 0.4;

/// Draw distinct cards the way a won fight does.
///
/// `CardRarityOdds.Roll`, which moves the run's own offset: a rare puts it
/// back where it started, and anything else nudges the next roll towards one.
/// That is the difference from [`regular`], which reads the base odds and
/// leaves the run alone.
///
/// Each card still costs three draws: the rarity, the card at it, and the
/// upgrade roll.
pub(crate) fn encounter<V: Rules, R: GameRng>(
    rng: &mut R,
    odds: &mut Odds,
    ascension: V::AscensionLevel,
    character: V::Character,
    count: usize,
    act: usize,
) -> Vec<V::Card> {
    if super::combat::scarce::<V>(ascension) {
        unimplemented!("the reward odds a scarce run rolls");
    }
    if act > 0 {
        // `RollForUpgrade` weighs the act index, so the first act upgrades
        // nothing and every act after it upgrades something.
        unimplemented!("an upgraded card in a reward");
    }

    let cards = V::reward_cards(character);
    let mut drawn = Vec::with_capacity(count);
    for _ in 0..count {
        let wanted = rarity(rng, odds);
        let choices = available::<V>(cards, &drawn, wanted);
        let left = choices.iter().filter(|card| !drawn.contains(card)).count() as i32;
        let bound = Bound::new(left).expect("an available rarity has a card");
        let at = rng.next_int(bound) as usize;
        drawn.push(
            choices
                .iter()
                .filter(|card| !drawn.contains(card))
                .nth(at)
                .copied()
                .expect("the draw is inside the count"),
        );
        let _upgrade_roll = rng.next_double();
    }
    drawn
}

/// One rarity, and the offset it leaves behind.
fn rarity<R: GameRng>(rng: &mut R, odds: &mut Odds) -> RewardRarity {
    let rolled = rng.next_double() as f32;
    let rare_below = RARE + odds.rarity;
    let wanted = if rolled < rare_below {
        RewardRarity::Rare
    } else if rolled < UNCOMMON + rare_below {
        RewardRarity::Uncommon
    } else {
        RewardRarity::Common
    };
    odds.rarity = match wanted {
        RewardRarity::Rare => OFFSET_BASE,
        _ => (odds.rarity + OFFSET_GROWTH).min(OFFSET_MAX),
    };
    wanted
}
