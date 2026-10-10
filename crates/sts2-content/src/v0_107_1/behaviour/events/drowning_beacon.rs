//! Drowning Beacon: a Glowwater Potion, or Fresnel Lens for Max HP.

use nonmax::NonMaxU8;
use sts2_core::{CustomReward, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::potions::Potion;
use super::super::super::generated::relics::Relic;

/// `HpLossVar(13)`, which the climb takes off the maximum.
const MAX_HP_LOSS: i32 = 13;

/// `BottleOption`'s one `PotionReward(GlowwaterPotion)`.
const BOTTLE: [CustomReward<V>; 1] = [CustomReward::SpecificPotion(Potion::GlowwaterPotion)];

#[derive(Debug, Clone)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct DrowningBeacon;

impl EventBehavior<V> for DrowningBeacon {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State { chosen: None }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::DrowningBeacon,
            &[
                (Said::DrowningBeaconBottle, false),
                (Said::DrowningBeaconClimb, false),
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
            0 => smallvec::smallvec![HookEffect::OfferRewards(&BOTTLE), HookEffect::FinishEvent],
            _ => smallvec::smallvec![
                HookEffect::LoseMaxHp(MAX_HP_LOSS),
                HookEffect::ObtainRelic(Relic::FresnelLens),
                HookEffect::FinishEvent,
            ],
        }
    }
}
