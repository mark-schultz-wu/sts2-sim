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
    let pool = V::reward_potions(character);
    let rolled = rng.next_double() as f32;
    let wanted = if rolled <= RARE {
        RewardRarity::Rare
    } else if rolled <= UNCOMMON {
        RewardRarity::Uncommon
    } else {
        RewardRarity::Common
    };
    at::<V, R>(pool, wanted, rng)
}

/// One of the pool's potions at a rarity.
fn at<V: Rules, R: GameRng>(
    pool: Drafted<'_, V::Potion>,
    wanted: RewardRarity,
    rng: &mut R,
) -> V::Potion {
    let choices = pool.at(wanted);
    let count = num_traits::cast(choices.len())
        .unwrap_or_else(|| panic!("a potion pool fits the RNG integer"));
    let bound = Bound::try_new(count).unwrap_or_else(|_| unimplemented!("a rarity with no potion"));
    let at: usize = num_traits::cast(rng.next_int(bound))
        .unwrap_or_else(|| unreachable!("a bounded draw is a valid potion index"));
    choices[at]
}
