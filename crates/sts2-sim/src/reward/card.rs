//! Card rewards drawn from a player's reward stream.

use sts2_core::rng::{Bound, GameRng};
use sts2_core::{Drafted, RewardRarity, Rules};

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
