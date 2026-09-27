//! Potion Courier: three Foul Potions, or one uncommon potion.

use nonmax::NonMaxU8;
use sts2_core::{CustomReward, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::potions::Potion;

/// `DynamicVar("FoulPotions", 3)`: a `PotionReward` of a Foul Potion each.
const FOUL: [CustomReward<V>; 3] = [CustomReward::SpecificPotion(Potion::FoulPotion); 3];

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct PotionCourier;

impl EventBehavior<V> for PotionCourier {
    type State = State;

    /// `CurrentActIndex > 0`, which the shared eligibility check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::PotionCourier,
            &[
                (Said::PotionCourierGrabPotions, false),
                (Said::PotionCourierRansack, false),
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
            0 => &FOUL,
            _ => &[CustomReward::PotionOf(sts2_core::RewardRarity::Uncommon)],
        };
        smallvec::smallvec![HookEffect::OfferRewards(offered), HookEffect::FinishEvent]
    }
}
