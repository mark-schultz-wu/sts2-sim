//! The engine rules this build spells its own way, which the simulator asks
//! through [`sts2_core::Engine`].

use sts2_core::RewardRarity;

use super::V0_111_0;

impl sts2_core::Engine for V0_111_0 {
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
