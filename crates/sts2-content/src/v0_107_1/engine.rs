//! The engine rules this build spells its own way, which the simulator asks
//! through [`sts2_core::Engine`].

use sts2_core::RewardRarity;

use super::V0_107_1;

impl sts2_core::Engine for V0_107_1 {
    /// A rare below the rare odds, then an uncommon below the uncommon odds
    /// alone.
    fn roll_with_base_odds(rolled: f32, rare: f32, uncommon: f32) -> RewardRarity {
        if rolled < rare {
            RewardRarity::Rare
        } else if rolled < uncommon {
            RewardRarity::Uncommon
        } else {
            RewardRarity::Common
        }
    }
}
