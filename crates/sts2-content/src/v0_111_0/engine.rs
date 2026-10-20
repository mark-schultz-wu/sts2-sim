//! The engine rules this build spells its own way, which the simulator asks
//! through [`sts2_core::Engine`].

use sts2_core::RewardRarity;

use super::V0_111_0;

impl sts2_core::Engine for V0_111_0 {
    /// A forced reward pays at once, with no draw, and leaves the odds where
    /// they were. v0.107.1's drew first and moved them.
    fn roll_potion_reward(
        current: f32,
        elite: bool,
        forced: bool,
        next_float: impl FnOnce() -> f32,
    ) -> (bool, f32) {
        use crate::v0_107_1::engine::{POTION_STEP, potion_bonus};

        if forced {
            return (true, current);
        }
        let against = current + potion_bonus(elite) * 0.5;
        match next_float() < against {
            true => (true, current - POTION_STEP),
            false => (false, current + POTION_STEP),
        }
    }

    /// A rare below the rare odds, then an uncommon below the rare and
    /// uncommon odds added together. v0.107.1's held the uncommon to its own
    /// odds alone.
    fn roll_with_base_odds(rolled: f32, rare: f32, uncommon: f32) -> RewardRarity {
        if rolled < rare {
            RewardRarity::Rare
        } else if rolled < uncommon + rare {
            RewardRarity::Uncommon
        } else {
            RewardRarity::Common
        }
    }
}
