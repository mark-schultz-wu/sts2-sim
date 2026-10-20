//! The engine rules this build spells its own way, which the simulator asks
//! through [`sts2_core::Engine`].

use sts2_core::RewardRarity;

use super::V0_107_1;

/// What `PotionRewardOdds.Roll` moves its odds by.
pub(crate) const POTION_STEP: f32 = 0.1;

/// The bonus an elite adds, a quarter, halved where it is used.
pub(crate) fn potion_bonus(elite: bool) -> f32 {
    match elite {
        true => 0.25,
        false => 0.0,
    }
}

impl sts2_core::Engine for V0_107_1 {
    /// The hand is listed, then the choice is signalled.
    fn lists_hand_after_pause() -> bool {
        false
    }

    /// The draw comes first, and a forced reward pays whatever it drew, with
    /// the odds moved as for a potion paid.
    fn roll_potion_reward(
        current: f32,
        elite: bool,
        forced: bool,
        next_float: impl FnOnce() -> f32,
    ) -> (bool, f32) {
        let against = current + potion_bonus(elite) * 0.5;
        let rolled = next_float();
        match forced || rolled < against {
            true => (true, current - POTION_STEP),
            false => (false, current + POTION_STEP),
        }
    }

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
