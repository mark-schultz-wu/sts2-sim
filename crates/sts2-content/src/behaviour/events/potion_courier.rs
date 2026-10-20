//! Potion Courier: three Foul Potions, or one uncommon potion.

use crate::ids::{EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{CustomReward, EventBehavior, EventView, HookEffect, HookEffects, Room};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct PotionCourier;

impl<V: crate::Build> EventBehavior<V> for PotionCourier {
    type State = State;

    /// `CurrentActIndex > 0`, which the shared eligibility check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::POTION_COURIER,
            &[
                (V::EventOption::POTION_COURIER_GRAB_POTIONS, false),
                (V::EventOption::POTION_COURIER_RANSACK, false),
            ],
            state.chosen,
        )
    }

    /// `GrabPotions`: `RewardsCmd.OfferCustom` of three Foul Potions.
    /// `Ransack`: one uncommon potion by `NextItem` off the rewards stream.
    /// Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let offered: &'static [CustomReward<V>] = match option {
            0 => V::POTION_COURIER_FOUL,
            _ => &[CustomReward::PotionOf(sts2_core::RewardRarity::Uncommon)],
        };
        smallvec::smallvec![HookEffect::OfferRewards(offered), HookEffect::FinishEvent]
    }
}

/// `FOUL`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `DynamicVar("FoulPotions", 3)`: a `PotionReward` of a Foul Potion each.
        const POTION_COURIER_FOUL: &'static [sts2_core::CustomReward<$build>] =
            &[sts2_core::CustomReward::SpecificPotion(
                <$build as sts2_core::ContentVersion>::Potion::FOUL_POTION,
            ); 3];
    };
}

pub(crate) use lists;
