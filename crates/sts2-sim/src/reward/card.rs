//! Card rewards drawn from a player's reward stream.

use sts2_core::rng::{Bound, GameRng};
use sts2_core::{RewardRarity, Rules};

/// Draw distinct cards with regular-encounter rarity odds.
///
/// Each card costs three draws: rarity, the item in that rarity, and the
/// upgrade roll. At Neow the upgrade chance is zero, but the roll still happens.
pub(crate) fn regular<V: Rules, R: GameRng>(
    rng: &mut R,
    cards: &[V::Card],
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
        let rarity = next_available::<V>(cards, &drawn, wanted);
        let choices: Vec<V::Card> = cards
            .iter()
            .copied()
            .filter(|card| !drawn.contains(card) && V::reward_rarity(*card) == rarity)
            .collect();
        let bound = Bound::new(choices.len() as i32).expect("an available rarity has a card");
        drawn.push(choices[rng.next_int(bound) as usize]);
        let _upgrade_roll = rng.next_double();
    }
    drawn
}

fn next_available<V: Rules>(
    cards: &[V::Card],
    drawn: &[V::Card],
    wanted: RewardRarity,
) -> RewardRarity {
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
        .find(|rarity| {
            cards
                .iter()
                .any(|card| !drawn.contains(card) && V::reward_rarity(*card) == *rarity)
        })
        .expect("a card pool has a drafted rarity")
}
