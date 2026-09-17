//! The potion a reward screen offers.
//!
//! `PotionFactory.CreateRandomPotion`, which is two draws: a rarity, and one
//! of the pool's potions at it.

use sts2_core::rng::{Bound, GameRng};
use sts2_core::{Drafted, RewardRarity, Rules};

/// A roll at or under this is rare, and at or under the second is uncommon.
///
/// Both are `<=` in the game, which matters at a draw of exactly one of them.
const RARE: f32 = 0.1;
const UNCOMMON: f32 = 0.35;

/// One potion, drawn the way a reward screen draws one.
///
/// Nothing is blacklisted. The blacklist is for a screen that offers more than
/// one, and no fight does.
pub(crate) fn rolled<V: Rules, R: GameRng>(rng: &mut R, character: V::Character) -> V::Potion {
    let [potion] = distinct::<V, R>(rng, character, 1)[..] else {
        unreachable!("one draw returns one potion")
    };
    potion
}

/// Several potions, no two the same.
///
/// `CreateRandomPotion` takes each one out of the list it drew it from. So a
/// later draw rolls a rarity and then indexes into what is left at it. A
/// merchant's three shelves are the only caller that asks for more than one.
pub(crate) fn distinct<V: Rules, R: GameRng>(
    rng: &mut R,
    character: V::Character,
    count: usize,
) -> Vec<V::Potion> {
    let pool = V::reward_potions(character);
    let mut drawn = Vec::with_capacity(count);
    for _ in 0..count {
        let rolled = rng.next_double() as f32;
        let wanted = if rolled <= RARE {
            RewardRarity::Rare
        } else if rolled <= UNCOMMON {
            RewardRarity::Uncommon
        } else {
            RewardRarity::Common
        };
        drawn.push(at::<V, R>(pool, wanted, &drawn, rng));
    }
    drawn
}

/// One of the pool's potions at a rarity.
fn at<V: Rules, R: GameRng>(
    pool: Drafted<'_, V::Potion>,
    wanted: RewardRarity,
    drawn: &[V::Potion],
    rng: &mut R,
) -> V::Potion {
    let choices: Vec<V::Potion> = pool
        .at(wanted)
        .iter()
        .copied()
        .filter(|potion| !drawn.contains(potion))
        .collect();
    let count = num_traits::cast(choices.len()).expect("a potion pool fits the RNG integer");
    let bound = Bound::try_new(count).unwrap_or_else(|_| unimplemented!("a rarity with no potion"));
    let at: usize =
        num_traits::cast(rng.next_int(bound)).expect("a bounded draw is a valid potion index");
    choices[at]
}

/// Which rarity band a potion sits in, read off the pool it was drawn from.
///
/// `PotionModel.Rarity`, which a merchant prices by. The drafted split is that
/// rarity. The pool is grouped by it, and a potion in a run came out of one of
/// these three groups.
pub(crate) fn rarity_of<V: Rules>(character: V::Character, potion: V::Potion) -> RewardRarity {
    let pool = V::reward_potions(character);
    if pool.rare.contains(&potion) {
        RewardRarity::Rare
    } else if pool.uncommon.contains(&potion) {
        RewardRarity::Uncommon
    } else {
        RewardRarity::Common
    }
}
