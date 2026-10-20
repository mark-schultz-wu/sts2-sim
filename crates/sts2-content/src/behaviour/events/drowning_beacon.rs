//! Drowning Beacon: a Glowwater Potion, or Fresnel Lens for Max HP.

use crate::ids::{EventIds as _, EventOptionIds as _, RelicIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `HpLossVar(13)`, which the climb takes off the maximum.
const MAX_HP_LOSS: i32 = 13;

#[derive(Debug, Clone)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct DrowningBeacon;

impl<V: crate::Build> EventBehavior<V> for DrowningBeacon {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State { chosen: None }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::DROWNING_BEACON,
            &[
                (V::EventOption::DROWNING_BEACON_BOTTLE, false),
                (V::EventOption::DROWNING_BEACON_CLIMB, false),
            ],
            state.chosen,
        )
    }

    /// `BottleOption`: a Glowwater Potion offered as a reward. `ClimbOption`:
    /// its Max HP lost, then Fresnel Lens. Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::OfferRewards(V::DROWNING_BEACON_BOTTLE),
                HookEffect::FinishEvent
            ],
            _ => smallvec::smallvec![
                HookEffect::LoseMaxHp(MAX_HP_LOSS),
                HookEffect::ObtainRelic(V::Relic::FRESNEL_LENS),
                HookEffect::FinishEvent,
            ],
        }
    }
}

/// `BOTTLE`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `BottleOption`'s one `PotionReward(GlowwaterPotion)`.
        const DROWNING_BEACON_BOTTLE: &'static [sts2_core::CustomReward<$build>] =
            &[sts2_core::CustomReward::SpecificPotion(
                <$build as sts2_core::ContentVersion>::Potion::GLOWWATER_POTION,
            )];
    };
}

pub(crate) use lists;
