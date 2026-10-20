//! Hungry for Mushrooms: one of two mushrooms, each a relic.

use crate::ids::EventIds as _;
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct HungryForMushrooms;

impl<V: crate::Build> EventBehavior<V> for HungryForMushrooms {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    /// Each option keyed by its page, and showing its relic.
    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let mut room = crate::behaviour::events::page::<V>(
            V::Event::HUNGRY_FOR_MUSHROOMS,
            &V::HUNGRY_FOR_MUSHROOMS_OFFERED
                .iter()
                .map(|&(said, _)| (said, false))
                .collect::<Vec<_>>(),
            state.chosen,
        );
        if let Room::Event { options, .. } = &mut room {
            for (option, (_, relic)) in options
                .iter_mut()
                .zip(V::HUNGRY_FOR_MUSHROOMS_OFFERED.iter().copied())
            {
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
        let (_, relic) = V::HUNGRY_FOR_MUSHROOMS_OFFERED[usize::from(option)];
        smallvec::smallvec![HookEffect::ObtainRelic(relic), HookEffect::FinishEvent]
    }
}

/// `OFFERED`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `GenerateInitialOptions`: a `RelicOption` for each, in this order.
        const HUNGRY_FOR_MUSHROOMS_OFFERED: &'static [(<$build as sts2_core::ContentVersion>::EventOption, <$build as sts2_core::ContentVersion>::Relic)] = &[
    (<$build as sts2_core::ContentVersion>::EventOption::HUNGRY_FOR_MUSHROOMS_BIG_MUSHROOM, <$build as sts2_core::ContentVersion>::Relic::BIG_MUSHROOM),
    (
        <$build as sts2_core::ContentVersion>::EventOption::HUNGRY_FOR_MUSHROOMS_FRAGRANT_MUSHROOM,
        <$build as sts2_core::ContentVersion>::Relic::FRAGRANT_MUSHROOM,
    ),
];
    };
}

pub(crate) use lists;
