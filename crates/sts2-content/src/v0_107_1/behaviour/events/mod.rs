//! Event behavior dispatch for this build.

mod stone_of_all_time;
mod tea_master;

use sts2_core::{EventBehavior, EventContext, EventFlow, EventView, Room};

use super::super::V0_107_1 as V;
use super::super::generated::events::Event;

#[derive(Debug, Clone)]
pub enum State {
    TeaMaster(tea_master::State),
}

pub(crate) fn allowed(event: Event, run: &impl EventView<V>) -> bool {
    match event {
        Event::StoneOfAllTime => stone_of_all_time::allowed(run),
        Event::TeaMaster => tea_master::TeaMaster.allowed(run),
        _ => unimplemented!("event eligibility: {event:?}"),
    }
}

pub(crate) fn enter<R: sts2_core::rng::GameRng>(
    event: Event,
    run: &impl EventView<V>,
    rng: &mut R,
) -> State {
    match event {
        Event::TeaMaster => State::TeaMaster(tea_master::TeaMaster.enter(run, rng)),
        _ => unimplemented!("entering event {event:?}"),
    }
}

pub(crate) fn observe(state: &State, run: &impl EventView<V>) -> Room<V> {
    match state {
        State::TeaMaster(state) => tea_master::TeaMaster.observe(state, run),
    }
}

pub(crate) fn choose<R: sts2_core::rng::GameRng, C: EventContext<V>>(
    state: &mut State,
    option: u8,
    run: &mut C,
    rng: &mut R,
) -> EventFlow<V> {
    match state {
        State::TeaMaster(state) => tea_master::TeaMaster.choose(state, option, run, rng),
    }
}
