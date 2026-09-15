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
        let (rare, uncommon) = RarityOdds::Regular.bands();
        let roll = rng.next_double() as f32;
        let wanted = if roll < rare {
            RewardRarity::Rare
        } else if roll < uncommon + rare {
            RewardRarity::Uncommon
        } else {
            RewardRarity::Common
        };
        let choices = available::<V>(cards, &drawn, wanted);
        let left = choices.iter().filter(|card| !drawn.contains(card)).count();
        let left = num_traits::cast(left).expect("an available rarity fits the RNG integer");
        let bound = Bound::try_new(left).expect("an available rarity has a card");
        let at =
            num_traits::cast(rng.next_int(bound)).expect("a bounded draw is a valid card index");
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

/// `CardRarityOddsType`, as the bands each one rolls against.
///
/// `GetBaseOdds` names three numbers per type and the roll reads two. Those
/// are the chance of a rare, and the chance of an uncommon added on top of it.
/// The third is whatever is left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RarityOdds {
    /// `RegularEncounter`, which a monster room's reward rolls.
    Regular,
    Elite,
    Boss,
}

impl RarityOdds {
    /// The chance of a rare, then of an uncommon.
    fn bands(self) -> (f32, f32) {
        match self {
            RarityOdds::Regular => (0.03, 0.37),
            RarityOdds::Elite => (0.1, 0.4),
            // A boss pays a rare and nothing else.
            RarityOdds::Boss => (1.0, 0.0),
        }
    }

    /// What the run's own offset adds to the chance of a rare.
    ///
    /// `Roll` passes zero for a boss, which is already paying one, and the
    /// run's running value for every other.
    fn offset(self, running: f32) -> f32 {
        match self {
            RarityOdds::Boss => 0.0,
            _ => running,
        }
    }
}

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
    bands: RarityOdds,
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
        let wanted = rarity(rng, odds, bands);
        let choices = available::<V>(cards, &drawn, wanted);
        let left = choices.iter().filter(|card| !drawn.contains(card)).count();
        let left = num_traits::cast(left).expect("an available rarity fits the RNG integer");
        let bound = Bound::try_new(left).expect("an available rarity has a card");
        let at =
            num_traits::cast(rng.next_int(bound)).expect("a bounded draw is a valid card index");
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
fn rarity<R: GameRng>(rng: &mut R, odds: &mut Odds, bands: RarityOdds) -> RewardRarity {
    let (rare, uncommon) = bands.bands();
    let rolled = rng.next_double() as f32;
    let rare_below = rare + bands.offset(odds.rarity);
    let wanted = if rolled < rare_below {
        RewardRarity::Rare
    } else if rolled < uncommon + rare_below {
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
