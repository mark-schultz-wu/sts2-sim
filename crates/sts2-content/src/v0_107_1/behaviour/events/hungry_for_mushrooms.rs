//! Hungry for Mushrooms: one of two mushrooms, each a relic.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::relics::Relic;

/// `GenerateInitialOptions`: a `RelicOption` for each, in this order.
const OFFERED: [(Said, Relic); 2] = [
    (Said::HungryForMushroomsBigMushroom, Relic::BigMushroom),
    (
        Said::HungryForMushroomsFragrantMushroom,
        Relic::FragrantMushroom,
    ),
];

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct HungryForMushrooms;

impl EventBehavior<V> for HungryForMushrooms {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    /// Each option keyed by its page, and showing its relic.
    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let mut room = super::page(
            Event::HungryForMushrooms,
            &OFFERED.map(|(said, _)| (said, false)),
            state.chosen,
        );
        if let Room::Event { options, .. } = &mut room {
            for (option, (_, relic)) in options.iter_mut().zip(OFFERED) {
                option.relic = Some(relic);
            }
        }
        room
    }

    /// `RelicCmd.Obtain` of the option's mushroom, which finishes the event.
    /// The fragrant one's damage is its own pickup's.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let (_, relic) = OFFERED[usize::from(option)];
        smallvec::smallvec![HookEffect::ObtainRelic(relic), HookEffect::FinishEvent]
    }
}
